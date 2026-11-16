use super::*;
use engineroom_backend::{
    db::mode::Mode,
    review::{
        engine::{Cancellation, EnginePort, Task},
        types::{RawPosition, Result as ReviewResult, ReviewResult as GameResult},
    },
};
use std::{collections::VecDeque, time::Duration};
use tokio::sync::{Notify, Semaphore};

#[derive(Default)]
struct EmptyRepo;
impl Repository for EmptyRepo {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, ReviewResult<Vec<Option<RawPosition>>>> {
        Box::pin(async move { Ok(vec![None; fens.len()]) })
    }
    fn put<'a>(
        &'a self,
        _: &'a [RawPosition],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, ReviewResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a GameResult) -> Task<'a, ReviewResult<()>> {
        Box::pin(async { Ok(()) })
    }
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async {})
    }
}

struct ControlledFactory {
    lease: Arc<Semaphore>,
    events: mpsc::Sender<&'static str>,
    teardown: Arc<Notify>,
    finish: Arc<Notify>,
}
struct ControlledPort {
    _permit: tokio::sync::OwnedSemaphorePermit,
    events: mpsc::Sender<&'static str>,
    teardown: Arc<Notify>,
    finish: Arc<Notify>,
    lines: VecDeque<String>,
}
impl EngineFactory for ControlledFactory {
    fn acquire<'a>(
        &'a self,
        cancel: &'a Cancellation,
    ) -> Task<'a, ReviewResult<Box<dyn EnginePort>>> {
        Box::pin(async move {
            let permit = tokio::select! {
                _ = cancel.cancelled() => return Err(ReviewError::cancelled()),
                permit = self.lease.clone().acquire_owned() => permit.expect("test lease is open"),
            };
            cancel.check()?;
            let _ = self.events.send("acquired");
            Ok(Box::new(ControlledPort {
                _permit: permit,
                events: self.events.clone(),
                teardown: self.teardown.clone(),
                finish: self.finish.clone(),
                lines: [
                    "option name UCI_LimitStrength type check default false",
                    "option name UCI_Elo type spin default 1500 min 1320 max 3190",
                    "uciok",
                ]
                .map(str::to_owned)
                .into(),
            }) as Box<dyn EnginePort>)
        })
    }
}
impl EnginePort for ControlledPort {
    fn send<'a>(&'a mut self, command: &'a str) -> Task<'a, ReviewResult<()>> {
        if command == "isready" {
            self.lines.push_back("readyok".into());
        }
        if command.starts_with("go ") {
            self.lines
                .push_back("info depth 8 multipv 1 score cp 25 pv e2e4".into());
            let _ = self.events.send("searching");
        }
        Box::pin(async { Ok(()) })
    }
    fn next(&mut self) -> Task<'_, ReviewResult<String>> {
        Box::pin(async move {
            if let Some(line) = self.lines.pop_front() {
                return Ok(line);
            }
            self.finish.notified().await;
            Ok("bestmove e2e4".into())
        })
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        Box::pin(async move {
            let _ = self.events.send("teardown");
            self.teardown.notified().await;
            let _ = self.events.send("terminated");
        })
    }
}
fn setup() -> (
    Bridge,
    mpsc::Receiver<&'static str>,
    Arc<Notify>,
    Arc<Notify>,
    mpsc::Receiver<()>,
) {
    setup_with_slots(1)
}
fn setup_with_slots(
    slots: usize,
) -> (
    Bridge,
    mpsc::Receiver<&'static str>,
    Arc<Notify>,
    Arc<Notify>,
    mpsc::Receiver<()>,
) {
    let (events, rx) = mpsc::channel();
    let teardown = Arc::new(Notify::new());
    let finish = Arc::new(Notify::new());
    let factory = ControlledFactory {
        lease: Arc::new(Semaphore::new(slots)),
        events,
        teardown: teardown.clone(),
        finish: finish.clone(),
    };
    let (wake, wakes) = mpsc::channel();
    let bridge = Bridge::with_ports(
        Arc::new(factory),
        Arc::new(EmptyRepo),
        Arc::new(move || {
            let _ = wake.send(());
        }),
    )
    .expect("create bridge");
    (bridge, rx, teardown, finish, wakes)
}
struct UnblockOnDrop(Arc<Notify>, Arc<Notify>);
impl Drop for UnblockOnDrop {
    fn drop(&mut self) {
        self.0.notify_one();
        self.1.notify_one();
    }
}
fn event(rx: &mpsc::Receiver<&'static str>, expected: &str) {
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("waiting for {expected}: {error}")),
        expected
    );
}

#[test]
fn cancel_one_queued_or_running_match_does_not_cancel_another() {
    let (bridge, events, teardown, finish, wake) = setup();
    let _unblock = UnblockOnDrop(teardown.clone(), finish.clone());
    bridge.play_move(1, 1500, "position startpos".into());
    event(&events, "acquired");
    event(&events, "searching");
    bridge.play_move(2, 1500, "position startpos".into());
    bridge.cancel_play(1);
    event(&events, "teardown");
    // A second process cannot own the lease until teardown completes.
    assert!(
        events.try_recv().is_err(),
        "lease released before termination"
    );
    teardown.notify_one();
    event(&events, "terminated");
    let acquired = events.recv_timeout(Duration::from_secs(5));
    if acquired.as_deref() == Ok("acquired") {
        event(&events, "searching");
        finish.notify_one();
        event(&events, "teardown");
        teardown.notify_one();
        event(&events, "terminated");
        wake.recv_timeout(Duration::from_secs(5))
            .expect("move wakes UI");
    }
    let data = bridge.drain_data();
    drop(bridge);
    assert_eq!(
        acquired.as_deref(),
        Ok("acquired"),
        "cancelling request 1 must leave request 2 runnable"
    );
    assert!(
        data.iter()
            .any(|e| matches!(e, DataEvent::PlayMove(2, Ok(m)) if m == "e2e4")),
        "second match must receive its move"
    );
    assert!(
        !data.iter().any(|e| matches!(e, DataEvent::PlayMove(1, _))),
        "cancelled match must not publish"
    );
}

#[test]
fn drop_cancels_search_and_waits_for_engine_termination() {
    let (bridge, events, teardown, _finish, _wake) = setup();
    bridge.play_move(10, 1500, "position startpos".into());
    event(&events, "acquired");
    event(&events, "searching");
    let (done, dropped) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        drop(bridge);
        let _ = done.send(());
    });
    event(&events, "teardown");
    assert!(
        dropped.try_recv().is_err(),
        "drop returned before termination"
    );
    teardown.notify_one();
    event(&events, "terminated");
    dropped
        .recv_timeout(Duration::from_secs(5))
        .expect("drop completes after termination");
    thread.join().expect("drop thread");
}

#[test]
fn completed_live_review_releases_lease_for_background_match() {
    use engineroom_backend::review::{
        core,
        types::{AnalysisKind, EngineTier, Event},
    };
    let (bridge, events, teardown, finish, wake) = setup();
    let _unblock = UnblockOnDrop(teardown.clone(), finish.clone());
    let pgn = "1. e4 e5 *";
    let game = core::extract(pgn).expect("valid PGN");
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let result = core::build(&game, &raw).expect("restored review");
    let fen = result.positions[0].fen.clone();
    bridge
        .open(
            "review",
            ReviewConfig {
                pgn: pgn.into(),
                meta: game.meta,
                engine: EngineTier {
                    id: "test".into(),
                    depth: 3,
                    label: String::new(),
                    hint: String::new(),
                },
                mode: Mode::Depth,
                analysis_kind: AnalysisKind::Manual,
                movetime_ms: None,
                lines: 1,
                initial_result: Some(result),
            },
        )
        .expect("open review");
    bridge
        .analyze(
            "review",
            1,
            LiveRequest {
                fen,
                variation_node_id: None,
                source_fen: None,
                source_analysis: None,
            },
            LiveSettings {
                search_seconds: 1,
                lines: 1,
                threads_auto: false,
                threads: 1,
                memory_mb: 16,
                move_feedback_enabled: false,
                fast_pass: false,
            },
        )
        .expect("start live analysis");
    event(&events, "acquired");
    event(&events, "searching");
    bridge.play_move(22, 1500, "position startpos".into());
    finish.notify_one();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if bridge
            .drain()
            .iter()
            .any(|e| matches!(e.event, Event::LiveCompleted { .. }))
        {
            break;
        }
        wake.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("live completion");
    }
    let releasing = events.recv_timeout(Duration::from_secs(5));
    if releasing.as_deref() == Ok("teardown") {
        assert!(
            events.try_recv().is_err(),
            "match started before review teardown"
        );
        teardown.notify_one();
        event(&events, "terminated");
        event(&events, "acquired");
        event(&events, "searching");
        finish.notify_one();
        event(&events, "teardown");
        teardown.notify_one();
        event(&events, "terminated");
    } else {
        // Unblock cleanup if this version retains the review port until close.
        bridge.close("review".into());
        if releasing.is_err() {
            event(&events, "teardown");
        }
        teardown.notify_one();
        event(&events, "terminated");
        // Cancel a possibly queued match before dropping the bridge.
        bridge.cancel_play(22);
        teardown.notify_one();
    }
    drop(bridge);
    assert_eq!(
        releasing.as_deref(),
        Ok("teardown"),
        "idle review must release the engine without being closed"
    );
}

fn review_config(restored: bool) -> ReviewConfig {
    use engineroom_backend::review::{
        core,
        types::{AnalysisKind, EngineTier},
    };
    let pgn = crate::game::DEMO_PGN;
    let game = core::extract(pgn).expect("fixture PGN");
    ReviewConfig {
        pgn: pgn.into(),
        meta: game.meta,
        engine: EngineTier {
            id: "qa".into(),
            depth: 8,
            label: String::new(),
            hint: String::new(),
        },
        mode: Mode::Time,
        analysis_kind: AnalysisKind::Manual,
        movetime_ms: Some(50),
        lines: 1,
        initial_result: restored.then(crate::fixtures::result),
    }
}
fn wait_for_bridge(
    bridge: &Bridge,
    wake: &mpsc::Receiver<()>,
    mut accept: impl FnMut(Vec<engineroom_backend::review::types::Envelope>) -> bool,
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    loop {
        if accept(bridge.drain()) {
            return;
        }
        wake.recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
            .expect("bridge event before deadline");
    }
}

#[test]
fn two_live_reviews_search_concurrently_and_closing_one_keeps_the_other_running() {
    use engineroom_backend::review::types::Event;
    let (bridge, events, teardown, finish, wake) = setup_with_slots(2);
    let _unblock = UnblockOnDrop(teardown.clone(), finish.clone());
    for id in ["one", "two"] {
        bridge.open(id, review_config(true)).expect("open review");
    }
    let mut completed = std::collections::HashSet::new();
    wait_for_bridge(&bridge, &wake, |events| {
        for e in events {
            if matches!(e.event, Event::Completed { .. }) {
                completed.insert(e.session_id);
            }
        }
        completed.len() == 2
    });
    for id in ["one", "two"] {
        bridge
            .analyze(
                id,
                1,
                LiveRequest {
                    fen: crate::fixtures::result().positions[0].fen.clone(),
                    variation_node_id: None,
                    source_fen: None,
                    source_analysis: None,
                },
                LiveSettings {
                    search_seconds: 1,
                    lines: 1,
                    threads_auto: false,
                    threads: 1,
                    memory_mb: 16,
                    move_feedback_enabled: false,
                    fast_pass: false,
                },
            )
            .expect("live review");
    }
    let mut acquired = 0;
    let mut searching = 0;
    while acquired < 2 || searching < 2 {
        match events
            .recv_timeout(Duration::from_secs(5))
            .expect("both reviews start before either completes")
        {
            "acquired" => acquired += 1,
            "searching" => searching += 1,
            other => panic!("unexpected event {other}"),
        }
    }
    bridge.close("one".into());
    event(&events, "teardown");
    teardown.notify_one();
    event(&events, "terminated");
    finish.notify_one();
    wait_for_bridge(&bridge, &wake, |events| {
        events
            .iter()
            .any(|e| e.session_id == "two" && matches!(e.event, Event::LiveCompleted { .. }))
    });
    event(&events, "teardown");
    teardown.notify_one();
    event(&events, "terminated");
    bridge.close("two".into());
}

#[test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH"]
fn real_stockfish_multiple_reviews_and_background_match_complete_together() {
    use engineroom_backend::review::types::Event;
    let path = std::env::var_os("ENGINE_ROOM_STOCKFISH").expect("set real Stockfish path");
    let (tx, wake) = mpsc::channel();
    let bridge = Bridge::with_ports(
        Arc::new(NativeEngineFactory::with_concurrency(path.into(), 2)),
        Arc::new(EmptyRepo),
        Arc::new(move || {
            let _ = tx.send(());
        }),
    )
    .expect("bridge");
    bridge.play_move(91, 1500, "position startpos".into());
    for id in ["one", "two", "three"] {
        bridge.open(id, review_config(false)).expect("open");
    }
    let mut completed = std::collections::HashSet::new();
    let mut played = false;
    wait_for_bridge(&bridge, &wake, |events| {
        for e in events {
            match e.event {
                Event::Completed { .. } => {
                    completed.insert(e.session_id);
                }
                Event::Error { error, .. } => panic!("review failed: {error}"),
                _ => {}
            }
        }
        for event in bridge.drain_data() {
            if let DataEvent::PlayMove(91, result) = event {
                let text = result.expect("background move");
                let mut game = crate::play::Match::new(shakmaty::Color::Black, 1500);
                game.engine_move(&text).expect("legal engine move");
                played = true;
            }
        }
        completed.len() == 3 && played
    });
    for id in ["one", "two", "three"] {
        bridge.close(id.into());
    }
}

#[test]
fn duplicate_session_id_does_not_replace_or_silence_the_existing_session() {
    use engineroom_backend::review::types::Event;
    let (bridge, _events, _teardown, _finish, wake) = setup();
    bridge
        .open("same", review_config(true))
        .expect("first open");
    assert!(bridge.open("same", review_config(true)).is_err());
    wait_for_bridge(&bridge, &wake, |events| {
        events
            .iter()
            .any(|e| e.session_id == "same" && matches!(e.event, Event::Completed { .. }))
    });
    bridge.close("same".into());
}
