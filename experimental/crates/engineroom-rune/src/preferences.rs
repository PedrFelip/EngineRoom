//! Versioned native preferences with serialized, atomic file writes off the UI thread.
use crate::state::Preferences;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

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
    /// The seam keeps transient-failure recovery deterministic in tests: a zero
    /// backoff avoids wall-clock waits, and no I/O happens on the UI thread.
    pub(crate) fn open_with<W>(
        path: PathBuf,
        wake: impl Fn() + Send + 'static,
        mut writer: W,
        retry: Retry,
    ) -> (Self, Result<Option<Preferences>, String>)
    where
        W: FnMut(&std::path::Path, &Preferences) -> Result<(), String> + Send + 'static,
    {
        let initial = read(&path);
        let (sender, receiver) = mpsc::channel();
        let (tx, results) = mpsc::channel();
        let worker = thread::spawn(move || {
            let mut desired: Option<Preferences> = None;
            let mut backoff = retry.initial;
            let mut stopping = false;
            loop {
                if desired.is_none() {
                    match receiver.recv() {
                        Ok(Intent::Save(preferences)) => desired = Some(preferences),
                        Ok(Intent::Stop) | Err(_) => break,
                    }
                }
                // Coalesce saves that arrived while we waited or wrote: only the
                // newest snapshot matters, and a new value resets the backoff.
                loop {
                    match receiver.try_recv() {
                        Ok(Intent::Save(preferences)) => {
                            desired = Some(preferences);
                            backoff = retry.initial;
                        }
                        Ok(Intent::Stop) => {
                            stopping = true;
                            break;
                        }
                        Err(_) => break,
                    }
                }
                let Some(preferences) = desired.take() else {
                    break;
                };
                match writer(&path, &preferences) {
                    Ok(()) => {
                        // ACK confirms the exact snapshot that reached disk.
                        let _ = tx.send(Ok(preferences));
                        wake();
                        backoff = retry.initial;
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                        wake();
                        // Keep the same snapshot for a retry; the UI never
                        // re-enqueues, so there is no busy loop on the caller.
                        desired = Some(preferences);
                        if !stopping {
                            // Back off, but keep listening for a newer snapshot
                            // or Stop so coalescing still works during the wait.
                            let deadline = Instant::now() + backoff;
                            let remaining = deadline.saturating_duration_since(Instant::now());
                            if remaining.is_zero() {
                                backoff = backoff.saturating_mul(2).min(retry.max);
                            } else {
                                match receiver.recv_timeout(remaining) {
                                    Ok(Intent::Save(new)) => {
                                        desired = Some(new);
                                        backoff = retry.initial;
                                    }
                                    Ok(Intent::Stop)
                                    | Err(mpsc::RecvTimeoutError::Disconnected) => {
                                        // The failed snapshot still needs its final
                                        // flush before Stop ends the worker.
                                        stopping = true;
                                        continue;
                                    }
                                    Err(mpsc::RecvTimeoutError::Timeout) => {
                                        backoff = backoff.saturating_mul(2).min(retry.max);
                                    }
                                }
                            }
                        }
                    }
                }
                // On Stop, the attempt above already wrote the latest snapshot;
                // exit without retrying so Drop never blocks forever.
                if stopping {
                    break;
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
pub(crate) fn write(path: &std::path::Path, preferences: &Preferences) -> Result<(), String> {
    validate(preferences)?;
    let bytes = serde_json::to_vec_pretty(&Document {
        version: 1,
        preferences: preferences.clone(),
    })
    .map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, bytes).map_err(|e| format!("Salvar preferências: {e}"))?;
    std::fs::rename(&temporary, path).map_err(|e| format!("Substituir preferências: {e}"))
}
#[cfg(test)]
mod tests {
    use super::*;
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
    fn transient_write_failure_retries_the_same_snapshot_without_ui_changes() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let path = path();
        let attempts = Arc::new(AtomicUsize::new(0));
        let seen = attempts.clone();
        let (store, initial) = Store::open_with(
            path.clone(),
            || {},
            move |p, preferences| {
                if seen.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err("falha transitória".into())
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

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_error = false;
        let mut acknowledged = None;
        while Instant::now() < deadline && acknowledged.is_none() {
            for event in store.drain() {
                match event {
                    Ok(snapshot) => acknowledged = Some(snapshot),
                    Err(_) => saw_error = true,
                }
            }
            if acknowledged.is_none() {
                thread::sleep(Duration::from_millis(5));
            }
        }
        assert!(saw_error, "the first write should fail");
        assert_eq!(
            acknowledged,
            Some(desired.clone()),
            "the retry must confirm the same snapshot"
        );
        assert_eq!(read(&path).unwrap(), Some(desired));
        assert_eq!(attempts.load(Ordering::SeqCst), 2, "exactly one retry");
        drop(store);
        std::fs::remove_file(path).unwrap();
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
                    Err("transient failure".into())
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
