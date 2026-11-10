use super::*;
use engineroom_backend::{
    db::mode::Mode,
    review::{
        core,
        engine::{Cancellation, EnginePort, Task},
        types::*,
    },
};
use std::{
    sync::atomic::AtomicUsize,
    time::{Duration, Instant},
};

struct NoEngine;
impl EngineFactory for NoEngine {
    fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        Box::pin(async {
            Err(ReviewError::new(
                ReviewErrorCode::EngineSpawn,
                "test.acquire",
                "unexpected engine acquisition",
            ))
        })
    }
}
#[derive(Default)]
struct MemoryRepo {
    drains: AtomicUsize,
}
impl Repository for MemoryRepo {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: Mode,
        _: u32,
        _: u32,
    ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
        Box::pin(async move { Ok(vec![None; fens.len()]) })
    }
    fn put<'a>(&'a self, _: &'a [RawPosition], _: Mode, _: u32, _: u32) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a ReviewResult) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn drain(&self) -> Task<'_, ()> {
        Box::pin(async {
            self.drains.fetch_add(1, Ordering::SeqCst);
        })
    }
}
fn config(restored: bool) -> ReviewConfig {
    let pgn = "1. e4 e5 *";
    let game = core::extract(pgn).expect("legal test PGN");
    let raw = game
        .fens
        .iter()
        .map(|f| core::terminal_raw(f, 0))
        .collect::<Vec<_>>();
    ReviewConfig {
        pgn: pgn.into(),
        meta: game.meta.clone(),
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
        initial_result: restored.then(|| core::build(&game, &raw).expect("valid restored review")),
    }
}
fn bridge(
    factory: Arc<dyn EngineFactory>,
    repo: Arc<dyn Repository>,
) -> (Bridge, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let bridge = Bridge::with_ports(
        factory,
        repo,
        Arc::new(move || {
            let _ = tx.send(());
        }),
    )
    .expect("create test runtime");
    (bridge, rx)
}
fn until(
    bridge: &Bridge,
    wake: &mpsc::Receiver<()>,
    mut check: impl FnMut(Vec<Envelope>, Vec<BridgeLifecycle>) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if check(bridge.drain(), bridge.drain_lifecycle()) {
            return;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "bridge event timeout");
        wake.recv_timeout(remaining)
            .expect("bridge wake before deadline");
    }
}

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "engineroom-bridge-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Bridge over a real temporary SQLite database and an injected engine.
fn data_bridge_with_engine(
    db: PathBuf,
    factory: Arc<dyn EngineFactory>,
) -> (Bridge, Arc<SqliteRepository>, mpsc::Receiver<()>) {
    let repo = Arc::new(SqliteRepository::new(db.clone()));
    let (tx, rx) = mpsc::channel();
    let bridge = Bridge::with_data_ports(
        factory,
        repo.clone(),
        Some(repo.clone()),
        Arc::new(move || {
            let _ = tx.send(());
        }),
        Arc::new(FakeImports),
        Some(db),
    )
    .expect("create test runtime");
    (bridge, repo, rx)
}

/// Bridge over a real temporary SQLite database and an engine that never runs.
fn data_bridge(db: PathBuf) -> (Bridge, Arc<SqliteRepository>, mpsc::Receiver<()>) {
    data_bridge_with_engine(db, Arc::new(NoEngine))
}

fn next_data(bridge: &Bridge, wake: &mpsc::Receiver<()>) -> DataEvent {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(event) = bridge.drain_data().pop() {
            return event;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "data event timeout");
        wake.recv_timeout(remaining)
            .expect("bridge wake before deadline");
    }
}

fn wait_storage(
    bridge: &Bridge,
    wake: &mpsc::Receiver<()>,
) -> engineroom_backend::db::stats::StorageStats {
    match next_data(bridge, wake) {
        DataEvent::Storage(Ok(stats)) => stats,
        DataEvent::Storage(Err(error)) => panic!("storage failed: {error}"),
        _ => panic!("expected storage event"),
    }
}

fn wait_cleared(bridge: &Bridge, wake: &mpsc::Receiver<()>, history: bool) {
    match next_data(bridge, wake) {
        DataEvent::Cleared(Ok(()), cleared_history) => {
            assert_eq!(cleared_history, history, "clear kind");
        }
        DataEvent::Cleared(Err(error), _) => panic!("clear failed: {error}"),
        _ => panic!("expected cleared event"),
    }
}

fn wait_games(
    bridge: &Bridge,
    wake: &mpsc::Receiver<()>,
    append: bool,
) -> engineroom_backend::db::games::GamePage {
    match next_data(bridge, wake) {
        DataEvent::Games(Ok(page), got_append) => {
            assert_eq!(got_append, append, "pagination append flag");
            page
        }
        DataEvent::Games(Err(error), _) => panic!("list games failed: {error}"),
        _ => panic!("expected games event"),
    }
}

#[test]
fn missing_stockfish_does_not_block_bridge_creation_or_storage() {
    let missing = std::env::temp_dir().join(format!(
        "engineroom-missing-stockfish-{}",
        std::process::id()
    ));
    assert!(!missing.is_file(), "test requires an absent engine path");
    assert_eq!(engine_fallback(Some(missing.clone())), missing);

    let dir = temp_dir("missing-engine");
    let db = dir.join("engineroom.db");
    let (bridge, repo, wake) = data_bridge_with_engine(
        db.clone(),
        Arc::new(NativeEngineFactory::new(engine_fallback(Some(missing)))),
    );

    bridge.storage_stats().expect("enqueue storage stats");
    let stats = wait_storage(&bridge, &wake);
    assert!(
        stats.db_bytes > 0,
        "temporary database file was not measured"
    );
    assert!(db.is_file(), "bridge removed the experimental database");

    drop(bridge);
    drop(repo);
    std::fs::remove_dir_all(dir).expect("remove temp dir");
}

#[test]
fn storage_stats_reports_database_file_and_clears_data() {
    let dir = temp_dir("storage");
    let db = dir.join("engineroom.db");
    let (bridge, repo, wake) = data_bridge(db.clone());

    let saved = config(false);
    let game = core::extract(&saved.pgn).expect("legal test PGN");
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let reviewed = core::build(&game, &raw).expect("valid review");
    bridge
        .runtime
        .block_on(repo.save(&saved, &reviewed))
        .expect("save game");
    let cached = core::terminal_raw(&core::fen(&shakmaty::Chess::default()), 25);
    bridge
        .runtime
        .block_on(repo.put(&[cached], Mode::Depth, 1, 1))
        .expect("cache position");

    bridge.storage_stats().expect("enqueue storage stats");
    let stats = wait_storage(&bridge, &wake);
    assert!(stats.db_bytes > 0, "database file size was not reported");
    assert!(stats.cache_bytes > 0, "cache table size was not reported");
    assert!(stats.games_bytes > 0, "games table size was not reported");

    let id = bridge
        .runtime
        .block_on(repo.list_games(10, None))
        .expect("list saved games")
        .games[0]
        .id;

    bridge.clear_cache().expect("clear cache");
    wait_cleared(&bridge, &wake, false);
    bridge.storage_stats().expect("enqueue storage stats");
    let stats = wait_storage(&bridge, &wake);
    assert_eq!(stats.cache_bytes, 0, "cache rows survived clear_cache");
    assert!(stats.games_bytes > 0, "clear_cache removed saved games");

    bridge.delete_game(id).expect("delete game");
    assert!(matches!(
        next_data(&bridge, &wake),
        DataEvent::Deleted(_, Ok(()))
    ));
    bridge.list_games(None).expect("refresh games");
    let page = wait_games(&bridge, &wake, false);
    assert!(page.games.is_empty(), "deleted game still listed");
    assert_eq!(page.total, 0);

    bridge
        .runtime
        .block_on(repo.save(&saved, &reviewed))
        .expect("save game again");
    bridge.clear_history().expect("clear history");
    wait_cleared(&bridge, &wake, true);
    bridge.list_games(None).expect("list games after clear");
    let page = wait_games(&bridge, &wake, false);
    assert!(page.games.is_empty(), "cleared history still listed");
    assert_eq!(page.total, 0);

    bridge.runtime.block_on(repo.drain());
    drop(bridge);
    drop(repo);
    std::fs::remove_dir_all(dir).expect("remove temp dir");
}

#[test]
fn list_games_paginates_through_real_sqlite() {
    let dir = temp_dir("pages");
    let db = dir.join("engineroom.db");
    {
        let conn = engineroom_backend::db::open_file(&db).expect("open temp db");
        let store = engineroom_backend::db::games::Store::new(&conn);
        for i in 0..101 {
            store
                .save(&engineroom_backend::db::games::NewGame {
                    pgn: format!("1. e4 e5 {i} *"),
                    white: format!("White {i}"),
                    black: format!("Black {i}"),
                    result: "1-0".into(),
                    plies: 2,
                    engine_tier: "test".into(),
                    mode: Mode::Depth,
                    analysis_kind: "manual".into(),
                    depth: 3,
                    multipv: 1,
                    accuracy_white: 50.0,
                    accuracy_black: 50.0,
                    review_json: "{}".into(),
                })
                .expect("insert game");
        }
    }

    let (bridge, repo, wake) = data_bridge(db);
    bridge.list_games(None).expect("first page");
    let first = wait_games(&bridge, &wake, false);
    assert_eq!(first.total, 101);
    assert_eq!(first.games.len(), 100);
    assert_eq!(first.games.last().expect("last game").id, 2);
    let cursor = first.next_cursor.expect("second page cursor");

    bridge.list_games(Some(cursor)).expect("second page");
    let second = wait_games(&bridge, &wake, true);
    assert_eq!(second.total, 101);
    assert_eq!(second.games.len(), 1);
    assert_eq!(second.games[0].id, 1);
    assert!(second.next_cursor.is_none(), "unexpected third page");

    bridge
        .delete_game(second.games[0].id)
        .expect("delete last game");
    assert!(matches!(
        next_data(&bridge, &wake),
        DataEvent::Deleted(_, Ok(()))
    ));
    bridge.list_games(None).expect("refresh games");
    let page = wait_games(&bridge, &wake, false);
    assert_eq!(page.total, 100);
    assert_eq!(page.games.len(), 100);

    bridge.runtime.block_on(repo.drain());
    drop(bridge);
    drop(repo);
    std::fs::remove_dir_all(dir).expect("remove temp dir");
}

#[test]
fn replacement_filters_old_events_and_acknowledges_cleanup() {
    let repo = Arc::new(MemoryRepo::default());
    let (b, wake) = bridge(Arc::new(NoEngine), repo.clone());
    b.open("first", config(true)).unwrap();
    until(&b, &wake, |_, events| {
        events
            .iter()
            .any(|e| matches!(e, BridgeLifecycle::Opened { id } if id == "first"))
    });
    b.open("second", config(true)).unwrap();
    let mut closed = false;
    until(&b, &wake, |events, lifecycle| {
        assert!(events.iter().all(|e| e.session_id == "second"));
        for e in lifecycle {
            match e {
                BridgeLifecycle::Closed { id } if id == "first" => closed = true,
                BridgeLifecycle::Opened { id } if id == "second" => {
                    assert!(closed, "replacement opened before close ack");
                    return true;
                }
                _ => {}
            }
        }
        false
    });
    b.close("second".into());
    until(&b, &wake, |_, events| {
        events
            .iter()
            .any(|e| matches!(e, BridgeLifecycle::Closed { id } if id == "second"))
    });
    assert!(repo.drains.load(Ordering::SeqCst) >= 2);
}
#[test]
fn closing_immediately_and_invalid_open_are_acknowledged() {
    let (b, wake) = bridge(Arc::new(NoEngine), Arc::new(MemoryRepo::default()));
    b.open("cancelled", config(true)).unwrap();
    b.close("cancelled".into());
    until(&b, &wake, |events, lifecycle| {
        assert!(events.is_empty(), "cancelled generation reached UI");
        lifecycle
            .iter()
            .any(|e| matches!(e, BridgeLifecycle::Closed { id } if id == "cancelled"))
    });
    let mut invalid = config(true);
    invalid.lines = 0;
    b.open("invalid", invalid).unwrap();
    until(&b, &wake, |_, events| {
        events.iter().any(|e| matches!(e, BridgeLifecycle::Error { id, error } if id == "invalid" && error.code == ReviewErrorCode::InvalidPayload))
    });
}
#[test]
fn dropping_bridge_drains_owned_session() {
    let repo = Arc::new(MemoryRepo::default());
    let (b, wake) = bridge(Arc::new(NoEngine), repo.clone());
    b.open("owned", config(true)).unwrap();
    until(&b, &wake, |_, events| {
        events
            .iter()
            .any(|e| matches!(e, BridgeLifecycle::Opened { .. }))
    });
    drop(b);
    assert!(
        repo.drains.load(Ordering::SeqCst) >= 1,
        "drop returned before repository drain"
    );
}
#[test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH; tests the real review state and variation feedback"]
fn real_stockfish_state_displays_live_lines_and_variation_feedback() {
    let engine = PathBuf::from(
        std::env::var_os("ENGINE_ROOM_STOCKFISH").expect("set ENGINE_ROOM_STOCKFISH"),
    );
    let dir = temp_dir("state-live");
    let repo = Arc::new(SqliteRepository::new(dir.join("test.db")));
    let (bridge, wake) = bridge(Arc::new(NativeEngineFactory::new(engine)), repo);
    let mut state = crate::state::State::default();
    state.bridge = Some(bridge);
    state.pgn.replace_text("1. e4 e5 *");
    state.prepare();
    state.profile = crate::state::Profile::Manual;
    state.depth = 3;
    state.preferences.search_seconds = 1;
    state.preferences.lines = 2;
    state.preferences.threads_auto = false;
    state.preferences.threads = 1;
    state.preferences.hash_mb = 16;
    state.start_review();
    let deadline = Instant::now() + Duration::from_secs(40);
    while state.real_result.is_none() {
        state.poll_bridge();
        assert!(
            state.bridge_error.is_none(),
            "review failed: {:?}",
            state.bridge_error
        );
        assert!(Instant::now() < deadline, "review timed out");
        let _ = wake.recv_timeout(Duration::from_millis(100));
    }
    assert!(
        !state.candidates().is_empty(),
        "game review should supply candidate lines"
    );
    state.click_square(52); // e2
    state.click_square(36); // e4
    assert!(state.active_variation.is_some());
    while state.displayed_classification().is_none() || state.live_positions.is_empty() {
        state.poll_bridge();
        assert!(
            state.bridge_error.is_none(),
            "review failed: {:?}",
            state.bridge_error
        );
        assert!(
            state.live_error.is_none(),
            "live analysis failed: {:?}",
            state.live_error
        );
        assert!(
            Instant::now() < deadline,
            "live lines or variation feedback timed out"
        );
        let _ = wake.recv_timeout(Duration::from_millis(100));
    }
    assert!(
        !state.candidates().is_empty(),
        "variation must show legal candidate lines"
    );
    assert!(state.displayed_classification().is_some());
    state.preferences.live_engine = false;
    state.poll_bridge();
    assert!(!state.live_running);
    state.go_to(1);
    state.preferences.live_engine = true;
    state.poll_bridge();
    state.go_to(2);
    state.poll_bridge();
    let latest_fen = core::fen(state.position());
    while !state.live_positions.contains_key(&latest_fen) {
        state.poll_bridge();
        assert!(
            state.live_error.is_none(),
            "latest search failed: {:?}",
            state.live_error
        );
        assert!(
            Instant::now() < deadline,
            "latest position search timed out"
        );
        let _ = wake.recv_timeout(Duration::from_millis(100));
    }
    assert_eq!(state.position_analysis().unwrap().fen, latest_fen);
    state.cancel_review();
    drop(state);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH; runs native bridge with temporary SQLite"]
fn real_stockfish_bridge_completion_live_cancel_and_reopen() {
    let engine = PathBuf::from(
        std::env::var_os("ENGINE_ROOM_STOCKFISH").expect("set ENGINE_ROOM_STOCKFISH"),
    );
    let dir = std::env::temp_dir().join(format!(
        "engineroom-bridge-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = Arc::new(SqliteRepository::new(dir.join("test.db")));
    let (b, wake) = bridge(Arc::new(NativeEngineFactory::new(engine)), repo.clone());
    b.open("real", config(false)).unwrap();
    let mut result = None;
    until(&b, &wake, |events, lifecycle| {
        assert!(
            !lifecycle
                .iter()
                .any(|e| matches!(e, BridgeLifecycle::Error { .. })),
            "bridge failure: {lifecycle:?}"
        );
        for e in events {
            match e.event {
                Event::Completed { result: r } => result = Some(r),
                Event::Error { error, .. } => panic!("review failed: {error}"),
                _ => {}
            }
        }
        result.is_some()
    });
    let result = result.unwrap();
    b.analyze(
        "real",
        1,
        LiveRequest {
            fen: result.positions[0].fen.clone(),
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
    .unwrap();
    until(&b, &wake, |events, _| {
        events
            .iter()
            .any(|e| e.request_id == Some(1) && matches!(e.event, Event::LiveCompleted { .. }))
    });
    b.cancel_live("real".into(), 2);
    b.close("real".into());
    until(&b, &wake, |_, events| {
        events
            .iter()
            .any(|e| matches!(e, BridgeLifecycle::Closed { id } if id == "real"))
    });
    let games = b.runtime.block_on(repo.list_games(10, None)).unwrap();
    assert!(!games.games.is_empty(), "completed game not saved");
    let saved = b
        .runtime
        .block_on(repo.review_config(games.games[0].id))
        .unwrap()
        .unwrap();
    assert!(saved.initial_result.is_some());
    b.open("reopened", saved).unwrap();
    until(&b, &wake, |events, _| {
        events
            .iter()
            .any(|e| matches!(e.event, Event::Completed { .. }))
    });
    drop(b);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn dropped_pgn_without_pointer_is_read_and_opens_configuration() {
    let (bridge, wake) = bridge(Arc::new(NoEngine), Arc::new(MemoryRepo::default()));
    let path = std::env::temp_dir().join(format!("engineroom-drop-{}.pgn", std::process::id()));
    std::fs::write(&path, "1. e4 e5 *").expect("write PGN");
    let mut rt = rune::App::new(crate::spec()).headless_runtime(
        (720.0, 560.0),
        rune::Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.state.bridge = Some(bridge);
    rt.state.import_source = crate::state::ImportSource::File;
    rt.invalidate();
    rt.frame(0.0);
    rt.drop_files(vec![path.clone()], None);
    rt.frame(0.02);
    wake.recv_timeout(Duration::from_secs(5))
        .expect("file read wake");
    rt.state.poll_bridge();
    assert_eq!(rt.state.page, crate::state::Page::Configure);
    assert_eq!(rt.state.pgn.text, "1. e4 e5 *");
    std::fs::remove_file(path).expect("remove PGN");
}

#[test]
fn file_import_runs_through_bridge_and_reports_errors() {
    let (bridge, wake) = bridge(Arc::new(NoEngine), Arc::new(MemoryRepo::default()));
    let path = std::env::temp_dir().join(format!("engineroom-import-{}.pgn", std::process::id()));
    std::fs::write(&path, "1. e4 e5 *").expect("write test PGN");
    bridge.read_file(path.clone()).expect("enqueue file read");
    wake.recv_timeout(Duration::from_secs(5))
        .expect("file read wake");
    assert_eq!(
        bridge
            .drain_files()
            .pop()
            .expect("file result")
            .expect("read PGN"),
        "1. e4 e5 *"
    );
    std::fs::remove_file(&path).expect("remove test PGN");
    bridge.read_file(path).expect("enqueue missing file");
    wake.recv_timeout(Duration::from_secs(5))
        .expect("missing file wake");
    assert!(bridge.drain_files().pop().expect("file result").is_err());
}

struct FakeImports;
impl engineroom_backend::imports::Http for FakeImports {
    fn get<'a>(
        &'a self,
        url: &'a str,
        _: usize,
    ) -> engineroom_backend::imports::Task<'a, engineroom_backend::imports::Result<Vec<u8>>> {
        Box::pin(async move {
            if url.ends_with("/archives") {
                Ok(br#"{"archives":[]}"#.to_vec())
            } else {
                panic!("unexpected URL: {url}")
            }
        })
    }
}
#[test]
fn import_bridge_search_cancel_and_replace() {
    use engineroom_backend::imports::{EventBody, SearchRequest};
    let (tx, rx) = mpsc::channel();
    let bridge = Bridge::with_data_ports(
        Arc::new(NoEngine),
        Arc::new(MemoryRepo::default()),
        None,
        Arc::new(move || {
            let _ = tx.send(());
        }),
        Arc::new(FakeImports),
        None,
    )
    .expect("test bridge");
    let request = |id: &str| SearchRequest {
        owner: "window".into(),
        request_id: id.into(),
        username: "player".into(),
        cursor: None,
    };
    bridge.search_chesscom(request("1")).expect("first search");
    bridge.cancel_chesscom("1".into());
    bridge.search_chesscom(request("2")).expect("replacement");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if bridge.drain_imports().into_iter().any(|event| {
            event.request_id == "2" && matches!(event.body, EventBody::Completed { .. })
        }) {
            break;
        }
        rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("replacement completes");
    }
}

#[test]
fn chess_selection_uses_the_result_pgn() {
    use engineroom_backend::imports::{Player, RemoteGame};
    let mut state = crate::state::State::default();
    state.chess_games.push(RemoteGame {
        url: "https://www.chess.com/game/live/1".into(),
        end_time: 1,
        time_class: None,
        white: Player {
            username: "White".into(),
            rating: None,
        },
        black: Player {
            username: "Black".into(),
            rating: None,
        },
        result: "*".into(),
        pgn: "1. e4 e5 *".into(),
    });
    state.select_chesscom(0);
    assert_eq!(state.pgn.text, "1. e4 e5 *");
    assert_eq!(
        state.game.avatar_key,
        Some(crate::avatar::game_key("1. e4 e5 *"))
    );
    assert_eq!(state.page, crate::state::Page::Configure);
    state.prepare();
    assert!(state.game.avatar_key.is_none());
}

struct PagedImports;
impl engineroom_backend::imports::Http for PagedImports {
    fn get<'a>(
        &'a self,
        url: &'a str,
        _: usize,
    ) -> engineroom_backend::imports::Task<'a, engineroom_backend::imports::Result<Vec<u8>>> {
        Box::pin(async move {
            if url.ends_with("/archives") {
                return Ok(
                    br#"{"archives":["https://api.chess.com/pub/player/player/games/2026/09"]}"#
                        .to_vec(),
                );
            }
            let games = (1..=21).map(|id| format!(r#"{{"url":"https://www.chess.com/game/live/{id}","end_time":{},"rules":"chess","white":{{"username":"White"}},"black":{{"username":"Black"}},"pgn":"[Result \"1-0\"]\n\n1. e4 e5 1-0"}}"#, 1700000000 + id)).collect::<Vec<_>>().join(",");
            Ok(format!(r#"{{"games":[{games}]}}"#).into_bytes())
        })
    }
}
#[test]
fn chess_controller_fetches_next_page() {
    let (tx, rx) = mpsc::channel();
    let bridge = Bridge::with_data_ports(
        Arc::new(NoEngine),
        Arc::new(MemoryRepo::default()),
        None,
        Arc::new(move || {
            let _ = tx.send(());
        }),
        Arc::new(PagedImports),
        None,
    )
    .expect("test bridge");
    let mut state = crate::state::State::default();
    state.bridge = Some(bridge);
    state.chess_username.replace_text("player");
    state.search_chesscom();
    for expected in [20, 21] {
        let deadline = Instant::now() + Duration::from_secs(5);
        while state.chess_loading {
            rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("search completes");
            state.poll_bridge();
        }
        assert_eq!(
            state.chess_games.len(),
            expected,
            "page results: {:?}",
            state.import_error
        );
        if expected == 20 {
            assert!(state.chess_cursor.is_some());
            state.next_chesscom();
        }
    }
    assert!(state.chess_cursor.is_none());
}

#[test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH; verifies workspace switching with native processes"]
fn real_stockfish_workspace_switch_during_analysis_preserves_completed_review() {
    use crate::state::{Page, Profile, State};
    let engine = PathBuf::from(std::env::var_os("ENGINE_ROOM_STOCKFISH").expect("set engine path"));
    let dir = std::env::temp_dir().join(format!(
        "engineroom-workspace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = Arc::new(SqliteRepository::new(dir.join("review.db")));
    let (b, wake) = bridge(Arc::new(NativeEngineFactory::new(engine)), repo);
    let mut state = State::default();
    state.bridge = Some(b);
    state.profile = Profile::Manual;
    state.depth = 3;
    state.pgn.replace_text("1. e4 e5 *");
    state.prepare();
    state.start_review();
    let deadline = Instant::now() + Duration::from_secs(20);
    while state.real_result.is_none() {
        state.poll_bridge();
        assert!(
            state.bridge_error.is_none(),
            "analysis error: {:?}",
            state.bridge_error
        );
        if state.real_result.is_some() {
            break;
        }
        wake.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("first review completion");
    }
    let first = state.active_workspace.unwrap();
    state.go_to(2);
    state.flipped = true;
    state.navigate(Page::Home);
    state.pgn.replace_text("1. d4 d5 2. c4 e6 *");
    state.prepare();
    state.depth = 30;
    state.start_review();
    let second = state.active_workspace.unwrap();
    assert_ne!(first, second);
    state.activate_review(first);
    assert_eq!(state.game.moves.len(), 2);
    assert_eq!(state.ply, 2);
    assert!(state.flipped);
    assert_eq!(state.real_result.as_ref().unwrap().positions.len(), 3);
    // Drop drains the queued open/close replacement, any live search, and history save.
    drop(state);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "requires ENGINE_ROOM_STOCKFISH; verifies interrupted review cache reuse"]
fn real_stockfish_bridge_resumes_interrupted_review_from_cache() {
    let engine = PathBuf::from(std::env::var_os("ENGINE_ROOM_STOCKFISH").expect("set engine path"));
    let dir = std::env::temp_dir().join(format!(
        "engineroom-cache-resume-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let repo = Arc::new(SqliteRepository::new(dir.join("review.db")));
    let (b, wake) = bridge(Arc::new(NativeEngineFactory::new(engine)), repo.clone());
    let mut review = config(false);
    review.pgn = crate::game::DEMO_PGN.into();
    let game = core::extract(&review.pgn).unwrap();
    review.meta = game.meta;
    review.mode = Mode::Time;
    review.movetime_ms = Some(200);
    b.open("interrupted", review.clone()).unwrap();
    until(&b, &wake, |events, _| {
        events.iter().any(|e| matches!(&e.event, Event::Progress{progress} if progress.completed >= 9 && progress.completed < progress.total))
    });
    b.open("other", config(true)).unwrap();
    until(&b, &wake, |_, events| {
        events
            .iter()
            .any(|e| matches!(e,BridgeLifecycle::Opened{id} if id=="other"))
    });
    let hits = b
        .runtime
        .block_on(repo.lookup(&game.fens, Mode::Time, 200, 1))
        .unwrap();
    let cached = hits.iter().filter(|h| h.is_some()).count();
    assert!(
        cached >= 8 && cached < game.fens.len(),
        "expected partial persisted cache, got {cached}"
    );
    b.open("resumed", review).unwrap();
    let mut reused = false;
    until(&b, &wake, |events, lifecycle| {
        assert!(
            !lifecycle
                .iter()
                .any(|e| matches!(e, BridgeLifecycle::Error { .. })),
            "bridge error: {lifecycle:?}"
        );
        for envelope in events {
            assert_eq!(
                envelope.session_id, "resumed",
                "stale generation escaped bridge"
            );
            match envelope.event {
                Event::Progress { progress } => reused |= progress.cached_positions > 0,
                Event::Completed { result } => {
                    assert_eq!(result.positions.len(), game.fens.len());
                    return true;
                }
                Event::Error { error, .. } => panic!("resume failed: {error}"),
                _ => {}
            }
        }
        false
    });
    assert!(reused, "resumed review never reported cache reuse");
    drop(b);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn probe_delivers_error_and_wakes_ui() {
    let (bridge, wake) = bridge(Arc::new(NoEngine), Arc::new(MemoryRepo::default()));
    bridge.start_probe();
    assert!(matches!(
        next_data(&bridge, &wake),
        DataEvent::Probe(Err(_))
    ));
}

struct WaitingProbe {
    started: mpsc::Sender<()>,
    stopped: Arc<AtomicUsize>,
}
struct WaitingPort(Arc<AtomicUsize>);
impl EngineFactory for WaitingProbe {
    fn acquire<'a>(&'a self, _: &'a Cancellation) -> Task<'a, Result<Box<dyn EnginePort>>> {
        let _ = self.started.send(());
        Box::pin(async { Ok(Box::new(WaitingPort(self.stopped.clone())) as Box<dyn EnginePort>) })
    }
}
impl EnginePort for WaitingPort {
    fn send<'a>(&'a mut self, _: &'a str) -> Task<'a, Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn next(&mut self) -> Task<'_, Result<String>> {
        Box::pin(std::future::pending())
    }
    fn shutdown(&mut self) -> Task<'_, ()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    }
}
#[test]
fn dropping_bridge_cancels_probe_and_awaits_teardown() {
    let (started, received) = mpsc::channel();
    let stopped = Arc::new(AtomicUsize::new(0));
    let (bridge, _) = bridge(
        Arc::new(WaitingProbe {
            started,
            stopped: stopped.clone(),
        }),
        Arc::new(MemoryRepo::default()),
    );
    bridge.start_probe();
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("probe started");
    drop(bridge);
    assert_eq!(stopped.load(Ordering::SeqCst), 1);
}

#[test]
fn history_keyboard_retry_recovers_initial_and_paginated_failures() {
    use rune::{Env, Runtime};
    fn settle(rt: &mut Runtime<crate::state::State>, wake: &mpsc::Receiver<()>) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while rt.state.history_loading {
            wake.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("history response before deadline");
            rt.state.poll_bridge();
        }
        rt.invalidate();
        rt.frame(0.0);
    }
    fn retry(rt: &mut Runtime<crate::state::State>) {
        rt.set_tab_visits_controls(true);
        for _ in 0..150 {
            if rt
                .elements()
                .iter()
                .any(|e| e.focused && e.key.as_deref() == Some("history-retry"))
            {
                rt.key(rune::KeyEvent {
                    key: rune::Key::Text(" ".into()),
                    mods: Default::default(),
                });
                rt.frame(0.0);
                assert!(
                    rt.state.history_loading,
                    "keyboard retry must enqueue a request"
                );
                return;
            }
            rt.key(rune::KeyEvent {
                key: rune::Key::Tab,
                mods: Default::default(),
            });
            rt.frame(0.0);
        }
        panic!("history retry must be keyboard reachable");
    }
    let dir = temp_dir("history-retry");
    let data_dir = dir.join("data");
    let db = data_dir.join("history.db");
    let (bridge, repo, wake) = data_bridge(db);
    let mut rt = rune::App::new(crate::spec()).headless_runtime(
        (1220.0, 900.0),
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.state = crate::state::State::fixture();
    rt.state.bridge = Some(bridge);
    rt.state.navigate(crate::state::Page::History);
    settle(&mut rt, &wake);
    assert!(
        rt.state.history_error.is_some(),
        "missing directory must fail initial load"
    );
    std::fs::create_dir(&data_dir).expect("restore database directory");
    retry(&mut rt);
    settle(&mut rt, &wake);
    assert!(rt.state.history_error.is_none());
    assert!(rt.state.history.is_empty());

    let saved = config(false);
    let game = core::extract(&saved.pgn).expect("legal PGN");
    let raw: Vec<_> = game
        .fens
        .iter()
        .map(|fen| core::terminal_raw(fen, 0))
        .collect();
    let reviewed = core::build(&game, &raw).expect("review");
    rt.state
        .bridge
        .as_ref()
        .expect("bridge")
        .runtime
        .block_on(repo.save(&saved, &reviewed))
        .expect("save game");
    rt.state.history_cursor = Some(engineroom_backend::db::games::GameCursor {
        created_at: "9999".into(),
        id: i64::MAX,
    });
    let moved = dir.join("offline");
    std::fs::rename(&data_dir, &moved).expect("make database unavailable");
    rt.state.next_history();
    settle(&mut rt, &wake);
    assert!(
        rt.state.history_error.is_some(),
        "pagination failure must be visible"
    );
    std::fs::rename(&moved, &data_dir).expect("restore database");
    retry(&mut rt);
    settle(&mut rt, &wake);
    assert!(rt.state.history_error.is_none());
    assert_eq!(
        rt.state.history.len(),
        1,
        "retry must append the saved game"
    );
    drop(rt);
    drop(repo);
    std::fs::remove_dir_all(dir).expect("remove temporary database");
}
