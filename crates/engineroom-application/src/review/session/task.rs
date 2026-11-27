//! Review execution, latest live intent and acknowledged teardown.
use super::{Resources, Session};
use crate::review::{
    engine::Cancellation,
    pipeline::{Pipeline, hash_mb},
    repository::normalize,
    types::{Envelope, Event, Progress, ReviewConfig, ReviewError, ReviewErrorCode},
};
use tokio::sync::watch;

pub(super) struct Completion {
    pub finished: watch::Sender<u64>,
    pub done: watch::Sender<bool>,
}

pub(super) async fn run_session(
    session_id: &str,
    config: &ReviewConfig,
    session: &Session,
    completion: Completion,
    mut pipeline: Pipeline,
    resources: Option<Resources>,
    publish_envelope: &mut (dyn FnMut(Envelope) -> bool + Send),
) {
    let Completion { finished, done } = completion;
    let root = Cancellation {
        closed: session.close.subscribe(),
        revision: None,
    };
    let mut sequence = 0;
    let mut resources_detected = false;
    let mut publish = |request_id, event| {
        if let Event::Warning { error } | Event::Error { error, .. } = &event {
            eprintln!("review failure: {error:?}");
        }
        sequence += 1;
        if !publish_envelope(Envelope {
            session_id: session_id.to_owned(),
            sequence,
            request_id,
            event,
        }) {
            session.close.send_replace(true);
        }
    };
    let result = if let Some(initial) = &config.initial_result {
        serde_json::to_value(initial)
            .map_err(|e| ReviewError::new(ReviewErrorCode::InvalidPayload, "session.restore", e))
            .and_then(normalize)
    } else {
        publish(
            None,
            Event::Progress {
                progress: Progress {
                    stage: "preparing".into(),
                    completed: 0,
                    total: config.meta.plies.saturating_add(1),
                    current_ply: 0,
                    phase: None,
                    cached_positions: 0,
                    engine_positions: 0,
                    remaining_budget_ms: None,
                    update: None,
                },
            },
        );
        let sizing = resources.map(|r| {
            pipeline.set_detected_resources(r.threads, r.memory_mb);
            resources_detected = true;
            (r.threads, hash_mb(r.memory_mb))
        });
        pipeline
            .review(config, sizing, &root, &mut |event| publish(None, event))
            .await
    };
    let ready = match result {
        Ok(result) => {
            if root.check().is_ok() {
                publish(
                    None,
                    Event::Completed {
                        result: result.clone(),
                    },
                );
                if config.initial_result.is_none()
                    && let Err(error) = pipeline.save(config, &result).await
                {
                    publish(None, Event::Warning { error });
                }
            }
            true
        }
        Err(error) => {
            if error.code != ReviewErrorCode::Cancelled {
                publish(None, Event::Error { error, fen: None });
            }
            false
        }
    };
    let mut revisions = session.revision.subscribe();
    let mut observed = 0;
    while root.check().is_ok() {
        let (id, job) = {
            let mut latest = match session.latest.lock() {
                Ok(latest) => latest,
                Err(error) => {
                    publish(
                        None,
                        Event::Error {
                            error: ReviewError::new(
                                ReviewErrorCode::Session,
                                "session.intent",
                                error,
                            ),
                            fen: None,
                        },
                    );
                    session.close.send_replace(true);
                    break;
                }
            };
            let id = *session.revision.borrow();
            let job = if id != observed { latest.take() } else { None };
            (id, job)
        };
        if id != observed {
            observed = id;
            if let Some(job) = job.filter(|_| ready) {
                let cancel = Cancellation {
                    closed: session.close.subscribe(),
                    revision: Some((session.revision.subscribe(), job.id)),
                };
                if job.settings.threads_auto
                    && !resources_detected
                    && let Some(r) = resources
                {
                    pipeline.set_detected_resources(r.threads, r.memory_mb);
                    resources_detected = true;
                }
                if let Err(error) = pipeline
                    .live(&job.request, &job.settings, &cancel, &mut |event| {
                        publish(Some(job.id), event)
                    })
                    .await
                    && error.code != ReviewErrorCode::Cancelled
                {
                    publish(
                        Some(job.id),
                        Event::Error {
                            error,
                            fen: Some(job.request.fen),
                        },
                    );
                }
            } else {
                pipeline.discard().await;
            }
            finished.send_replace(id);
            continue;
        }
        tokio::select! { _ = root.cancelled() => break, change = revisions.changed() => { if change.is_err() { break; } } }
    }
    pipeline.close().await;
    done.send_replace(true);
}
