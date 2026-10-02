use super::*;

#[tokio::test]
async fn closed_worker_reply_requires_actual_cancellation_and_preserves_failure() {
    let (closed, receiver) = tokio::sync::watch::channel(false);
    let cancel = Cancellation {
        closed: receiver,
        revision: None,
    };
    let (sender, reply) = tokio::sync::oneshot::channel::<()>();
    drop(sender);
    let error = reply
        .await
        .map_err(|_| disconnected_worker_error(None, &cancel))
        .unwrap_err();
    assert_eq!(error.code, "engineExited");

    closed.send_replace(true);
    assert_eq!(disconnected_worker_error(None, &cancel).code, "cancelled");
    let failure = ReviewError::new("engineProtocol", "test.worker", "original failure");
    let error = disconnected_worker_error(Some(failure), &cancel);
    assert_eq!(error.code, "engineProtocol");
    assert_eq!(error.message, "original failure");
}
