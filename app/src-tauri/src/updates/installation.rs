use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

/// Owns the file-replacement phase independently of the IPC command awaiting it.
#[derive(Debug, Default)]
pub(super) struct InstallationGate {
    active: Arc<Mutex<()>>,
    closing: AtomicBool,
}

/// The blocking installer owns this lease until its file operations have finished.
#[derive(Debug)]
pub(super) struct Installation {
    _active: OwnedMutexGuard<()>,
}

impl InstallationGate {
    pub(super) fn begin(&self) -> Result<Installation, &'static str> {
        let installation = self
            .active
            .clone()
            .try_lock_owned()
            .map_err(|_| "Uma atualização já está em andamento.")?;
        // Check after acquiring: close may race with a command finishing its download.
        if self.closing.load(Ordering::SeqCst) {
            return Err("O aplicativo está encerrando.");
        }
        Ok(Installation {
            _active: installation,
        })
    }

    pub(super) fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }

    pub(super) fn is_idle(&self) -> bool {
        self.active.try_lock().is_ok()
    }

    pub(super) async fn drain(&self) {
        self.close();
        let _installation = self.active.lock().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn exit_waits_for_file_replacement_even_after_command_is_dropped() {
        let gate = Arc::new(InstallationGate::default());
        let installation = gate.begin().unwrap();
        let (started, ready) = oneshot::channel();
        let (release, blocked) = mpsc::channel();
        let command = tokio::spawn(async move {
            tokio::task::spawn_blocking(move || {
                let _installation = installation;
                started.send(()).unwrap();
                blocked.recv().unwrap();
            })
            .await
            .unwrap();
        });
        ready.await.unwrap();
        command.abort();
        let _ = command.await;

        gate.close();
        assert!(!gate.is_idle(), "file replacement must keep exit blocked");
        assert!(gate.begin().is_err(), "exit must reject a new installation");
        let waiting_gate = gate.clone();
        let mut exit = tokio::spawn(async move { waiting_gate.drain().await });
        assert!(
            tokio::time::timeout(Duration::from_millis(30), &mut exit)
                .await
                .is_err(),
            "dropping the IPC waiter must not permit exit during file replacement"
        );
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(2), exit)
            .await
            .expect("exit must resume when file replacement finishes")
            .unwrap();
        assert!(
            gate.is_idle(),
            "completed installation must release its guard"
        );
        assert!(gate.begin().is_err(), "closing must remain permanent");
    }

    #[tokio::test]
    async fn failed_installation_releases_exit_and_allows_retry_before_close() {
        let gate = InstallationGate::default();
        let installation = gate.begin().unwrap();
        assert!(
            gate.begin().is_err(),
            "only one installation may own the gate"
        );
        let result = tokio::task::spawn_blocking(move || {
            let _installation = installation;
            Err::<(), _>("installer refused")
        })
        .await
        .unwrap();
        assert!(result.is_err(), "test must exercise an installer failure");
        let retry = gate.begin().expect("failed installation must allow retry");
        drop(retry);
        gate.close();
        gate.drain().await;
        assert!(gate.is_idle(), "failed installation must not block exit");
        assert!(
            gate.begin().is_err(),
            "exit must reject subsequent installs"
        );
    }
}
