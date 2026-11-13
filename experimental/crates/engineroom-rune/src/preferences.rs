//! Versioned native preferences with serialized, atomic file writes off the UI thread.
use crate::state::Preferences;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::mpsc, thread};

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
pub struct Store {
    sender: mpsc::Sender<Intent>,
    results: mpsc::Receiver<Result<Option<Preferences>, String>>,
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
        let initial = read(&path);
        let (sender, receiver) = mpsc::channel();
        let (tx, results) = mpsc::channel();
        let worker = thread::spawn(move || {
            while let Ok(intent) = receiver.recv() {
                let Intent::Save(mut preferences) = intent else {
                    break;
                };
                let mut stop = false;
                while let Ok(next) = receiver.try_recv() {
                    match next {
                        Intent::Save(new) => preferences = new,
                        Intent::Stop => {
                            stop = true;
                            break;
                        }
                    }
                }
                let _ = tx.send(write(&path, preferences).map(|()| None));
                wake();
                if stop {
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
    pub fn drain(&self) -> Vec<Result<Option<Preferences>, String>> {
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
fn write(path: &std::path::Path, preferences: Preferences) -> Result<(), String> {
    validate(&preferences)?;
    let bytes = serde_json::to_vec_pretty(&Document {
        version: 1,
        preferences,
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
        write(&path, p.clone()).unwrap();
        assert_eq!(read(&path).unwrap(), Some(p));
        let before = std::fs::read(&path).unwrap();
        let invalid = Preferences {
            lines: 0,
            ..Preferences::default()
        };
        assert!(write(&path, invalid).is_err());
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
}
