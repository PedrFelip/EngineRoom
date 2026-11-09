//! Non-blocking native PGN selection, with results delivered to the UI thread.

use std::{
    fmt,
    future::Future,
    io,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, mpsc},
    thread,
};

/// Outcome of one selection; `None` means the user cancelled.
pub type DialogResult = Result<Option<PathBuf>, DialogError>;
type DialogFuture = Pin<Box<dyn Future<Output = DialogResult> + Send>>;
type Picker = Arc<dyn Fn() -> DialogFuture + Send + Sync>;

/// Failure to start or run a dialog.
#[derive(Debug)]
pub struct DialogError(String);

impl DialogError {
    /// Creates an error from a backend-specific message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}
impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for DialogError {}

/// A dialog worker. Dropping it cancels pending futures without blocking the UI.
pub struct FileDialog {
    requests: mpsc::Sender<()>,
    results: mpsc::Receiver<DialogResult>,
}

impl fmt::Debug for FileDialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDialog").finish_non_exhaustive()
    }
}

impl FileDialog {
    /// Starts a worker and calls `wake` whenever a result becomes available.
    ///
    /// # Errors
    /// Returns an error if the worker thread cannot be started.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> io::Result<Self> {
        Self::with_picker(wake, || {
            Box::pin(async {
                Ok(rfd::AsyncFileDialog::new()
                    .add_filter("Chess games", &["pgn"])
                    .pick_file()
                    .await
                    .map(|file| file.path().to_owned()))
            })
        })
    }

    fn with_picker(
        wake: impl Fn() + Send + Sync + 'static,
        picker: impl Fn() -> DialogFuture + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let (requests, incoming) = mpsc::channel();
        let (outgoing, results) = mpsc::channel();
        let wake = Arc::new(wake);
        let picker: Picker = Arc::new(picker);
        thread::Builder::new()
            .name("file-dialog".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = outgoing.send(Err(DialogError::new(error.to_string())));
                        wake();
                        return;
                    }
                };
                // A separate runtime task per request lets teardown abort pending dialogs.
                while incoming.recv().is_ok() {
                    let outgoing = outgoing.clone();
                    let wake = Arc::clone(&wake);
                    let picker = Arc::clone(&picker);
                    runtime.spawn(async move {
                        if outgoing.send(picker().await).is_ok() {
                            wake();
                        }
                    });
                }
                runtime.shutdown_background();
            })?;
        Ok(Self { requests, results })
    }

    /// Queues a selection without waiting for the native dialog.
    ///
    /// # Errors
    /// Returns an error if the dialog worker has stopped.
    pub fn request(&self) -> Result<(), DialogError> {
        self.requests
            .send(())
            .map_err(|error| DialogError::new(error.to_string()))
    }

    /// Removes all completed selections in completion order.
    pub fn drain(&self) -> Vec<DialogResult> {
        self.results.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        time::{Duration, Instant},
    };

    use super::{DialogError, DialogFuture, DialogResult, FileDialog};

    /// Upper bound for worker wake-ups; keeps a broken worker from hanging the suite.
    const WAKE_TIMEOUT: Duration = Duration::from_secs(5);

    /// Builds a dialog backed by a fake picker and returns the wake receiver.
    fn dialog_with<F>(picker: F) -> (FileDialog, mpsc::Receiver<()>)
    where
        F: Fn() -> DialogFuture + Send + Sync + 'static,
    {
        let (wake, woken) = mpsc::channel();
        let wake = Arc::new(wake);
        let dialog = FileDialog::with_picker(
            move || {
                let _ = wake.send(());
            },
            picker,
        )
        .expect("dialog worker thread should start");
        (dialog, woken)
    }

    fn wait_for_wake(woken: &mpsc::Receiver<()>) {
        woken
            .recv_timeout(WAKE_TIMEOUT)
            .expect("dialog worker should wake within the timeout");
    }

    fn only_result(dialog: &FileDialog) -> DialogResult {
        let mut results = dialog.drain();
        assert_eq!(results.len(), 1, "expected exactly one completed dialog");
        results.pop().expect("length was checked above")
    }

    #[test]
    fn successful_selection_delivers_the_chosen_path() {
        let (dialog, woken) = dialog_with(|| {
            Box::pin(async { Ok::<_, DialogError>(Some(PathBuf::from("/tmp/partida.pgn"))) })
        });

        dialog.request().expect("queueing a request should succeed");
        wait_for_wake(&woken);

        let path = only_result(&dialog)
            .expect("selection should succeed")
            .expect("a file should be chosen");
        assert_eq!(path, PathBuf::from("/tmp/partida.pgn"));
    }

    #[test]
    fn cancellation_yields_no_path() {
        let (dialog, woken) = dialog_with(|| Box::pin(async { Ok::<_, DialogError>(None) }));

        dialog.request().expect("queueing a request should succeed");
        wait_for_wake(&woken);

        let chosen = only_result(&dialog).expect("cancellation is not an error");
        assert!(chosen.is_none(), "a cancelled dialog has no path");
    }

    #[test]
    fn backend_failure_is_reported() {
        let (dialog, woken) = dialog_with(|| {
            Box::pin(async { Err::<Option<PathBuf>, _>(DialogError::new("falhou")) })
        });

        dialog.request().expect("queueing a request should succeed");
        wait_for_wake(&woken);

        let error = only_result(&dialog).expect_err("a failed dialog should be an error");
        assert_eq!(error.to_string(), "falhou");
    }

    #[test]
    fn dropping_the_dialog_does_not_wait_for_a_pending_picker() {
        let (dialog, _woken) = dialog_with(|| Box::pin(pending::<DialogResult>()));

        dialog.request().expect("queueing a request should succeed");

        let started = Instant::now();
        drop(dialog);
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "dropping the dialog took {elapsed:?}, expected under 2s"
        );
    }

    #[test]
    fn two_requests_produce_two_independent_results() {
        let next = Arc::new(AtomicUsize::new(0));
        let (dialog, woken) = dialog_with({
            let next = Arc::clone(&next);
            move || {
                let index = next.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    Ok::<_, DialogError>(Some(PathBuf::from(format!("/tmp/{index}.pgn"))))
                })
            }
        });

        dialog.request().expect("first request should queue");
        dialog.request().expect("second request should queue");
        wait_for_wake(&woken);
        wait_for_wake(&woken);

        // `drain` reports completion order; the single-worker runtime may schedule
        // either task first, so only the set of results is asserted.
        let results = dialog.drain();
        assert_eq!(results.len(), 2, "expected two completed dialogs");
        let mut paths = results
            .into_iter()
            .map(|result| {
                result
                    .expect("both selections should succeed")
                    .expect("both selections should choose a file")
            })
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(
            paths,
            vec![PathBuf::from("/tmp/0.pgn"), PathBuf::from("/tmp/1.pgn")]
        );
    }
}
