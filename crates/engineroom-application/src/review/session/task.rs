//! Review execution, latest live intent and acknowledged teardown.
use super::{Resources, SchedulerMode, Session};
use crate::review::{
    adaptive::{self, Profile},
    automatic::{self, LiveDemand, NativeLive},
    core,
    engine::Cancellation,
    pipeline::{NativeLiveClient, NativePhase, Pipeline, hash_mb},
    repository::normalize,
    types::{AnalysisKind, Envelope, Event, Progress, ReviewConfig, ReviewError, ReviewErrorCode},
};
use std::sync::Mutex;
use tokio::sync::{mpsc, watch};

pub(super) struct Completion {
    pub finished: watch::Sender<u64>,
    pub done: watch::Sender<bool>,
}

struct Publisher<'a> {
    session_id: &'a str,
    close: &'a watch::Sender<bool>,
    sequence: u64,
    publish_envelope: &'a mut (dyn FnMut(Envelope) -> bool + Send),
}

impl Publisher<'_> {
    fn emit(&mut self, request_id: Option<u64>, event: Event) {
        if let Event::Warning { error } | Event::Error { error, .. } = &event {
            eprintln!("review failure: {error:?}");
        }
        self.sequence += 1;
        if !(self.publish_envelope)(Envelope {
            session_id: self.session_id.to_owned(),
            sequence: self.sequence,
            request_id,
            event,
        }) {
            self.close.send_replace(true);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "internal session entrypoint owns its concrete ports and channels"
)]
pub(super) async fn run_session(
    session_id: &str,
    config: &ReviewConfig,
    session: &Session,
    completion: Completion,
    mut pipeline: Pipeline,
    resources: Option<Resources>,
    scheduler: SchedulerMode,
    publish_envelope: &mut (dyn FnMut(Envelope) -> bool + Send),
) {
    let Completion { finished, done } = completion;
    let root = Cancellation {
        closed: session.close.subscribe(),
        revision: None,
    };
    if let Some(r) = resources {
        pipeline.set_detected_resources(r.threads, r.memory_mb);
    }
    let publisher = Mutex::new(Publisher {
        session_id,
        close: &session.close,
        sequence: 0,
        publish_envelope,
    });
    let emit = |request_id, event| {
        publisher
            .lock()
            .expect("session publisher lock poisoned")
            .emit(request_id, event);
    };
    if scheduler == SchedulerMode::Native
        && config.initial_result.is_none()
        && config.analysis_kind != AnalysisKind::Manual
    {
        emit(None, preparing_progress(config));
        match adaptive::profile(config.analysis_kind) {
            Ok(Some(profile)) => {
                run_native_automatic(
                    config,
                    session,
                    &root,
                    &finished,
                    &mut pipeline,
                    resources,
                    profile,
                    &emit,
                )
                .await;
                pipeline.close().await;
                done.send_replace(true);
                return;
            }
            Ok(None) => {}
            Err(error) => {
                if error.code != ReviewErrorCode::Cancelled {
                    emit(None, Event::Error { error, fen: None });
                }
                let (_phase_tx, phase_rx) = watch::channel(NativePhase::Failed);
                run_live_loop(
                    session,
                    &root,
                    &finished,
                    &mut pipeline,
                    None,
                    &phase_rx,
                    &emit,
                )
                .await;
                pipeline.close().await;
                done.send_replace(true);
                return;
            }
        }
    }
    let result = if let Some(initial) = &config.initial_result {
        serde_json::to_value(initial)
            .map_err(|e| ReviewError::new(ReviewErrorCode::InvalidPayload, "session.restore", e))
            .and_then(normalize)
    } else {
        emit(None, preparing_progress(config));
        let sizing = resources.map(|r| (r.threads, hash_mb(r.memory_mb)));
        pipeline
            .review(config, sizing, &root, &mut |event| emit(None, event))
            .await
    };
    let ready = match result {
        Ok(result) => {
            if root.check().is_ok() {
                emit(
                    None,
                    Event::Completed {
                        result: result.clone(),
                    },
                );
                if config.initial_result.is_none()
                    && let Err(error) = pipeline.save(config, &result).await
                {
                    emit(None, Event::Warning { error });
                }
            }
            true
        }
        Err(error) => {
            if error.code != ReviewErrorCode::Cancelled {
                emit(None, Event::Error { error, fen: None });
            }
            false
        }
    };
    let (_phase_tx, phase_rx) = watch::channel(if ready {
        NativePhase::Ready
    } else {
        NativePhase::Failed
    });
    run_live_loop(
        session,
        &root,
        &finished,
        &mut pipeline,
        None,
        &phase_rx,
        &emit,
    )
    .await;
    pipeline.close().await;
    done.send_replace(true);
}

fn preparing_progress(config: &ReviewConfig) -> Event {
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
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "internal native session runner coordinates review and live streams"
)]
async fn run_native_automatic(
    config: &ReviewConfig,
    session: &Session,
    root: &Cancellation,
    finished: &watch::Sender<u64>,
    pipeline: &mut Pipeline,
    resources: Option<Resources>,
    profile: Profile,
    emit: &(dyn Fn(Option<u64>, Event) + Send + Sync),
) {
    pipeline.discard().await;
    let sizing = resources.map(|r| (r.threads, hash_mb(r.memory_mb)));
    let factory = pipeline.factory_arc();
    let repository = pipeline.repository_arc();
    let (demands_tx, mut demands_rx) = mpsc::channel::<LiveDemand>(16);
    let (phase_tx, phase_rx) = watch::channel(NativePhase::Running);
    let review_future = async {
        let pgn = config.pgn.clone();
        let result = match tokio::task::spawn_blocking(move || core::extract(&pgn)).await {
            Ok(Ok(game)) => match root.check() {
                Ok(()) => {
                    let live = NativeLive {
                        cancel: root,
                        demands: &mut demands_rx,
                    };
                    automatic::review_native(
                        factory.as_ref(),
                        repository.as_ref(),
                        &game,
                        profile,
                        sizing,
                        live,
                        &mut |event| emit(None, event),
                    )
                    .await
                }
                Err(error) => Err(error),
            },
            Ok(Err(error)) => Err(error),
            Err(error) => Err(ReviewError::new(ReviewErrorCode::InvalidPgn, "pgn", error)),
        };
        match result {
            Ok(result) => {
                if root.check().is_ok() {
                    emit(
                        None,
                        Event::Completed {
                            result: result.clone(),
                        },
                    );
                    phase_tx.send_replace(NativePhase::Ready);
                    drop(demands_rx);
                    if let Err(error) = repository.save(config, &result).await {
                        emit(None, Event::Warning { error });
                    }
                } else {
                    phase_tx.send_replace(NativePhase::Failed);
                    drop(demands_rx);
                }
            }
            Err(error) => {
                if error.code != ReviewErrorCode::Cancelled {
                    emit(None, Event::Error { error, fen: None });
                }
                phase_tx.send_replace(NativePhase::Failed);
                drop(demands_rx);
            }
        }
    };
    let live_future = run_live_loop(
        session,
        root,
        finished,
        pipeline,
        Some(&demands_tx),
        &phase_rx,
        emit,
    );
    tokio::join!(review_future, live_future);
}

async fn run_live_loop(
    session: &Session,
    root: &Cancellation,
    finished: &watch::Sender<u64>,
    pipeline: &mut Pipeline,
    demands: Option<&mpsc::Sender<LiveDemand>>,
    phase: &watch::Receiver<NativePhase>,
    emit: &(dyn Fn(Option<u64>, Event) + Send + Sync),
) {
    let mut revisions = session.revision.subscribe();
    let mut observed = 0;
    while root.check().is_ok() {
        let (id, job) = {
            let mut latest = match session.latest.lock() {
                Ok(latest) => latest,
                Err(error) => {
                    emit(
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
            let current_phase = *phase.borrow();
            if let Some(job) = job.filter(|_| current_phase != NativePhase::Failed) {
                let cancel = Cancellation {
                    closed: session.close.subscribe(),
                    revision: Some((session.revision.subscribe(), job.id)),
                };
                let native = demands
                    .filter(|_| current_phase == NativePhase::Running)
                    .map(|demands| NativeLiveClient { demands, phase });
                if let Err(error) = pipeline
                    .live_with_native(&job.request, &job.settings, &cancel, native, &mut |event| {
                        if cancel.check().is_ok() {
                            emit(Some(job.id), event);
                        }
                    })
                    .await
                    && error.code != ReviewErrorCode::Cancelled
                    && cancel.check().is_ok()
                {
                    emit(
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
        tokio::select! {
            _ = root.cancelled() => break,
            change = revisions.changed() => {
                if change.is_err() {
                    break;
                }
            }
        }
    }
}
