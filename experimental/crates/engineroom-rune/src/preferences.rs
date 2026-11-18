//! Versioned native preferences with serialized, atomic file writes off the UI thread.
use crate::state::Preferences;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};

/// Maximum write attempts for one snapshot before it is reported as failed.
///
/// Bounds both transient retries and the final flush at shutdown so a broken
/// writer can never keep the worker (or `Store::drop`) alive indefinitely.
const MAX_WRITE_ATTEMPTS: usize = 3;

/// Why a preference write failed, classified without inspecting error strings.
#[derive(Debug)]
pub(crate) enum WriteError {
    /// Retrying the same snapshot cannot help (invalid data or serialization).
    Permanent(String),
    /// A later attempt may succeed (transient filesystem failure).
    Transient(String),
}
impl WriteError {
    /// Creates a failure that retrying the same snapshot cannot fix.
    pub(crate) fn permanent(message: impl Into<String>) -> Self {
        Self::Permanent(message.into())
    }
    /// Creates a failure that a later attempt may overcome.
    pub(crate) fn transient(message: impl Into<String>) -> Self {
        Self::Transient(message.into())
    }
    fn is_permanent(&self) -> bool {
        matches!(self, Self::Permanent(_))
    }
    fn into_message(self) -> String {
        match self {
            Self::Permanent(message) | Self::Transient(message) => message,
        }
    }
}
impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Permanent(message) | Self::Transient(message) => message,
        })
    }
}
impl std::error::Error for WriteError {}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    #[serde(default)]
    preferences: Preferences,
}

enum Intent {
    Save(Preferences),
    Stop,
}

/// Backoff for recoverable write failures. Retries stay on the worker thread.
pub(crate) struct Retry {
    pub(crate) initial: Duration,
    pub(crate) max: Duration,
}
impl Default for Retry {
    fn default() -> Self {
        Self {
            initial: Duration::from_millis(10),
            max: Duration::from_secs(2),
        }
    }
}
impl Retry {
    /// Zero backoff so transient-failure tests need no wall-clock waits.
    #[cfg(test)]
    pub(crate) fn immediate() -> Self {
        Self {
            initial: Duration::ZERO,
            max: Duration::ZERO,
        }
    }
}

pub struct Store {
    sender: mpsc::Sender<Intent>,
    results: mpsc::Receiver<Result<Preferences, String>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Store {
    /// Reads the current document before starting the save worker.
    ///
    /// The initial read is synchronous so setup can apply persisted values
    /// without racing the first UI frame; saves stay on the worker thread.
    pub fn open(
        path: PathBuf,
        wake: impl Fn() + Send + 'static,
    ) -> (Self, Result<Option<Preferences>, String>) {
        Self::open_with(path, wake, write, Retry::default())
    }

    /// Same as [`Store::open`] with an injectable writer and retry policy.
    ///
    /// The seam keeps failure recovery deterministic in tests: a zero backoff
    /// avoids wall-clock waits, and no I/O happens on the UI thread.
    pub(crate) fn open_with<W>(
        path: PathBuf,
        wake: impl Fn() + Send + 'static,
        mut writer: W,
        retry: Retry,
    ) -> (Self, Result<Option<Preferences>, String>)
    where
        W: FnMut(&Path, &Preferences) -> Result<(), WriteError> + Send + 'static,
    {
        let initial = read(&path);
        let (sender, receiver) = mpsc::channel();
        let (tx, results) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut pending: Option<Preferences> = None;
            let mut attempts = 0usize;
            let mut stopping = false;
            let mut backoff = retry.initial;
            loop {
                // Wait for the next snapshot while idle. Stop with nothing pending
                // ends the worker; a queued snapshot still gets its final flush.
                while pending.is_none() && !stopping {
                    match receiver.recv() {
                        Ok(Intent::Save(preferences)) => {
                            pending = Some(preferences);
                            attempts = 0;
                            backoff = retry.initial;
                        }
                        Ok(Intent::Stop) | Err(_) => stopping = true,
                    }
                }
                if pending.is_none() {
                    break;
                }
                // Only the newest snapshot matters. A replacement gets its own
                // attempt budget and backoff; Stop is remembered for teardown.
                if coalesce(&receiver, &mut pending, &mut stopping) {
                    attempts = 0;
                    backoff = retry.initial;
                }
                let Some(snapshot) = pending.take() else {
                    continue;
                };
                loop {
                    attempts += 1;
                    match writer(&path, &snapshot) {
                        Ok(()) => {
                            // ACK confirms the exact snapshot that reached disk.
                            let _ = tx.send(Ok(snapshot.clone()));
                            wake();
                            break;
                        }
                        Err(error) => {
                            let exhausted = attempts >= MAX_WRITE_ATTEMPTS;
                            if error.is_permanent() || exhausted {
                                // One terminal result per snapshot: transient retries
                                // emit no error and no wake.
                                let _ = tx.send(Err(error.into_message()));
                                wake();
                                break;
                            }
                            // Transient failure with budget left. Back off while
                            // watching for a newer snapshot or Stop; once shutdown
                            // started there is nothing to wait for.
                            let delay = if stopping { Duration::ZERO } else { backoff };
                            backoff = backoff.saturating_mul(2).min(retry.max);
                            let mut superseded = false;
                            match receiver.recv_timeout(delay) {
                                Ok(Intent::Save(new)) => {
                                    pending = Some(new);
                                    superseded = true;
                                }
                                Ok(Intent::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                    stopping = true;
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {}
                            }
                            superseded |= coalesce(&receiver, &mut pending, &mut stopping);
                            if superseded {
                                attempts = 0;
                                backoff = retry.initial;
                                break;
                            }
                        }
                    }
                }
            }
        });
        (
            Self {
                sender,
                results,
                worker: Some(worker),
            },
            initial,
        )
    }
    pub fn save(&self, preferences: Preferences) {
        let _ = self.sender.send(Intent::Save(preferences));
    }
    pub fn drain(&self) -> Vec<Result<Preferences, String>> {
        self.results.try_iter().collect()
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.sender.send(Intent::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
/// Keeps only the newest queued snapshot and remembers Stop. Returns whether a
/// newer snapshot replaced the current one (so its attempt budget must reset).
fn coalesce(
    receiver: &mpsc::Receiver<Intent>,
    pending: &mut Option<Preferences>,
    stopping: &mut bool,
) -> bool {
    let mut replaced = false;
    while let Ok(intent) = receiver.try_recv() {
        match intent {
            Intent::Save(preferences) => {
                *pending = Some(preferences);
                replaced = true;
            }
            Intent::Stop => *stopping = true,
        }
    }
    replaced
}
fn validate(p: &Preferences) -> Result<(), String> {
    if !p.volume.is_finite()
        || !(0.0..=1.0).contains(&p.volume)
        || !(1..=5).contains(&p.lines)
        || !(1..=256).contains(&p.threads)
        || !(16..=4096).contains(&p.hash_mb)
        || !(1..=30).contains(&p.search_seconds)
    {
        return Err("Preferências fora dos limites permitidos.".into());
    }
    Ok(())
}
fn read(path: &std::path::Path) -> Result<Option<Preferences>, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(Preferences::default()));
        }
        Err(e) => return Err(format!("Ler preferências: {e}")),
    };
    let doc: Document =
        serde_json::from_slice(&bytes).map_err(|e| format!("Preferências inválidas: {e}"))?;
    if doc.version != 1 {
        return Err(format!(
            "Versão de preferências não suportada: {}",
            doc.version
        ));
    }
    validate(&doc.preferences)?;
    Ok(Some(doc.preferences))
}
pub(crate) fn write(path: &Path, preferences: &Preferences) -> Result<(), WriteError> {
    validate(preferences).map_err(WriteError::permanent)?;
    let bytes = serde_json::to_vec_pretty(&Document {
        version: 1,
        preferences: preferences.clone(),
    })
    .map_err(|e| WriteError::permanent(e.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| WriteError::transient(e.to_string()))?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes)
        .map_err(|e| WriteError::transient(format!("Salvar preferências: {e}")))?;
    std::fs::rename(&temporary, path)
        .map_err(|e| WriteError::transient(format!("Substituir preferências: {e}")))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    fn path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "engineroom-settings-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn roundtrip_defaults_and_invalid_version_do_not_replace_existing_data() {
        let path = path();
        assert_eq!(read(&path).unwrap(), Some(Preferences::default()));
        let p = Preferences {
            sound: false,
            lines: 5,
            chess_username: "jogador".into(),
            ..Preferences::default()
        };
        write(&path, &p).unwrap();
        assert_eq!(read(&path).unwrap(), Some(p));
        let before = std::fs::read(&path).unwrap();
        let invalid = Preferences {
            lines: 0,
            ..Preferences::default()
        };
        assert!(write(&path, &invalid).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::write(&path, b"{\"version\":2}").unwrap();
        assert!(read(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn dropping_store_flushes_the_latest_queued_preferences() {
        let path = path();
        let (store, initial) = Store::open(path.clone(), || {});
        assert_eq!(initial.unwrap(), Some(Preferences::default()));
        let p = Preferences {
            hash_mb: 512,
            ..Preferences::default()
        };
        store.save(Preferences::default());
        store.save(p.clone());
        drop(store);
        assert_eq!(read(&path).unwrap(), Some(p));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn session_presentation_roundtrips_with_existing_preferences() {
        use crate::session_style::SessionColor;

        let path = path();
        std::fs::write(&path, br#"{"version":1,"preferences":{"sound":false}}"#).unwrap();
        let mut preferences = read(&path).unwrap().unwrap();
        assert!(
            !preferences.sound,
            "older documents keep their existing settings"
        );
        preferences
            .sessions
            .folders
            .push(crate::session_style::SessionFolder {
                id: "openings".into(),
                name: "Aberturas".into(),
                color: SessionColor::Violet,
            });
        preferences
            .sessions
            .collapsed_folders
            .insert("openings".into());
        write(&path, &preferences).unwrap();
        assert_eq!(read(&path).unwrap(), Some(preferences));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn permanent_write_failure_reports_once_without_retry() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = attempts.clone();
        let (wake_tx, wake_rx) = mpsc::channel();
        let (store, _) = Store::open_with(
            path(),
            move || {
                let _ = wake_tx.send(());
            },
            move |_, _| {
                seen.fetch_add(1, Ordering::SeqCst);
                Err(WriteError::permanent("invalid preferences"))
            },
            Retry::immediate(),
        );
        store.save(Preferences::default());
        wake_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("failure wake");
        let events = store.drain();
        assert_eq!(events.len(), 1, "one terminal result for the snapshot");
        assert_eq!(
            events[0].as_ref().err().map(String::as_str),
            Some("invalid preferences")
        );
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "a permanent failure must not be retried"
        );
    }

    #[test]
    fn transient_write_failure_retries_the_same_snapshot_without_repeated_errors() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = attempts.clone();
        let (wake_tx, wake_rx) = mpsc::channel();
        let (store, initial) = Store::open_with(
            path(),
            move || {
                let _ = wake_tx.send(());
            },
            move |p, preferences| {
                if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(WriteError::transient("falha transitória"))
                } else {
                    write(p, preferences)
                }
            },
            Retry::immediate(),
        );
        assert_eq!(initial.unwrap(), Some(Preferences::default()));
        let desired = Preferences {
            hash_mb: 512,
            ..Preferences::default()
        };
        store.save(desired.clone());
        wake_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("retry wake");
        let events = store.drain();
        assert_eq!(events.len(), 1, "one result for the snapshot");
        assert_eq!(
            events[0].as_ref().ok(),
            Some(&desired),
            "the retry confirms the same snapshot after its write"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "exactly one retry");
    }

    #[test]
    fn newer_snapshot_supersedes_a_failing_write_with_its_own_budget() {
        let (first_seen, saw_first) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let (written, saved) = mpsc::channel();
        let attempts = Arc::new(AtomicUsize::new(0));
        let tried = attempts.clone();
        let (wake_tx, wake_rx) = mpsc::channel();
        let (store, _) = Store::open_with(
            path(),
            move || {
                let _ = wake_tx.send(());
            },
            move |_, preferences| {
                tried.fetch_add(1, Ordering::SeqCst);
                if preferences.hash_mb == 512 {
                    first_seen.send(()).unwrap();
                    released.recv().unwrap();
                    Err(WriteError::transient("old snapshot still failing"))
                } else {
                    written.send(preferences.clone()).unwrap();
                    Ok(())
                }
            },
            Retry::immediate(),
        );
        let old = Preferences {
            hash_mb: 512,
            ..Preferences::default()
        };
        let new = Preferences {
            hash_mb: 1024,
            ..Preferences::default()
        };
        store.save(old);
        saw_first
            .recv_timeout(Duration::from_secs(5))
            .expect("first write started");
        store.save(new.clone());
        release.send(()).unwrap();
        assert_eq!(
            saved
                .recv_timeout(Duration::from_secs(5))
                .expect("new snapshot written"),
            new
        );
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            2,
            "one old attempt, then one fresh attempt for the new snapshot"
        );
        wake_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("new snapshot wake");
        let events = store.drain();
        assert_eq!(events.len(), 1, "only the newest snapshot is acknowledged");
        assert_eq!(events[0].as_ref().ok(), Some(&new));
    }

    #[test]
    fn stop_during_retry_backoff_flushes_the_pending_snapshot() {
        let (attempted, first_attempt) = mpsc::channel();
        let (written, snapshots) = mpsc::channel();
        let mut attempts = 0;
        let (store, _) = Store::open_with(
            path(),
            || {},
            move |_, preferences| {
                attempts += 1;
                if attempts == 1 {
                    attempted.send(()).unwrap();
                    Err(WriteError::transient("transient failure"))
                } else {
                    written.send(preferences.clone()).unwrap();
                    Ok(())
                }
            },
            Retry {
                initial: Duration::from_secs(60),
                max: Duration::from_secs(60),
            },
        );
        let desired = Preferences {
            hash_mb: 512,
            ..Preferences::default()
        };
        store.save(desired.clone());
        first_attempt
            .recv_timeout(Duration::from_secs(5))
            .expect("first write attempted");
        drop(store);
        assert_eq!(
            snapshots
                .try_recv()
                .expect("final flush after failed write"),
            desired
        );
        assert!(snapshots.try_recv().is_err(), "only one final flush");
    }
}
