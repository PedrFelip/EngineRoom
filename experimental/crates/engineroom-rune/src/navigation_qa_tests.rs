//! Adversarial navigation checks; registered by the crate's test harness.
use super::*;
use rune::{Env, Key, KeyEvent, Runtime};

fn runtime() -> Runtime<State> {
    let mut rt = rune::App::new(spec()).headless_runtime(
        (1220.0, 900.0),
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.state = State::fixture();
    rt.invalidate();
    rt.frame(0.0);
    rt
}

fn tap(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn settings_round_trip_preserves_review_and_import_draft() {
    let mut rt = runtime();
    let draft = "[Event \"unsaved\"]\n\n1. d4 d5 *";
    rt.state.pgn.replace_text(draft);
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-16");
    tap(&mut rt, "settings-nav");
    assert_eq!(rt.state.page, Page::Settings);
    tap(&mut rt, "settings-close");
    assert_eq!(rt.state.page, Page::Review);
    assert_eq!(rt.state.ply, 16);
    tap(&mut rt, "home-nav");
    assert_eq!(rt.state.pgn.text, draft);
}

#[test]
fn closing_review_while_settings_is_open_returns_home_with_draft() {
    let mut rt = runtime();
    let draft = "1. d4 d5 *";
    rt.state.pgn.replace_text(draft);
    tap(&mut rt, "session-1");
    tap(&mut rt, "settings-nav");
    rt.state.close_review(1);
    rt.invalidate();
    rt.frame(0.0);
    tap(&mut rt, "settings-close");
    assert_eq!(rt.state.page, Page::Home);
    assert_eq!(rt.state.pgn.text, draft);
    assert!(rt.state.workspace.is_empty());
}

#[test]
fn history_filter_input_does_not_navigate_review() {
    let mut rt = runtime();
    tap(&mut rt, "session-1");
    tap(&mut rt, "move-16");
    tap(&mut rt, "history-nav");
    tap(&mut rt, "history-search");
    rt.key(KeyEvent {
        key: Key::Text("f".into()),
        mods: Default::default(),
    });
    rt.key(KeyEvent {
        key: Key::Left,
        mods: Default::default(),
    });
    rt.frame(0.0);
    assert_eq!(rt.state.page, Page::History);
    assert_eq!(rt.state.history_filter.text, "f");
    tap(&mut rt, "session-1");
    assert_eq!(rt.state.ply, 16);
    assert!(!rt.state.flipped);
}

struct UnusedEngine;
impl engineroom_backend::review::engine::EngineFactory for UnusedEngine {
    fn acquire<'a>(
        &'a self,
        _: &'a engineroom_backend::review::engine::Cancellation,
    ) -> engineroom_backend::review::engine::Task<
        'a,
        engineroom_backend::review::types::Result<
            Box<dyn engineroom_backend::review::engine::EnginePort>,
        >,
    > {
        Box::pin(async {
            Err(engineroom_backend::review::types::ReviewError::new(
                engineroom_backend::review::types::ReviewErrorCode::EngineSpawn,
                "qa",
                "engine is not needed",
            ))
        })
    }
}

struct EmptyRepository;
impl engineroom_backend::review::repository::Repository for EmptyRepository {
    fn lookup<'a>(
        &'a self,
        fens: &'a [String],
        _: engineroom_backend::db::mode::Mode,
        _: u32,
        _: u32,
    ) -> engineroom_backend::review::engine::Task<
        'a,
        engineroom_backend::review::types::Result<
            Vec<Option<engineroom_backend::review::types::RawPosition>>,
        >,
    > {
        Box::pin(async move { Ok(vec![None; fens.len()]) })
    }
    fn put<'a>(
        &'a self,
        _: &'a [engineroom_backend::review::types::RawPosition],
        _: engineroom_backend::db::mode::Mode,
        _: u32,
        _: u32,
    ) -> engineroom_backend::review::engine::Task<'a, engineroom_backend::review::types::Result<()>>
    {
        Box::pin(async { Ok(()) })
    }
    fn save<'a>(
        &'a self,
        _: &'a engineroom_backend::review::types::ReviewConfig,
        _: &'a engineroom_backend::review::types::ReviewResult,
    ) -> engineroom_backend::review::engine::Task<'a, engineroom_backend::review::types::Result<()>>
    {
        Box::pin(async { Ok(()) })
    }
    fn drain(&self) -> engineroom_backend::review::engine::Task<'_, ()> {
        Box::pin(async {})
    }
}

#[test]
fn delayed_file_read_does_not_override_navigation_or_new_draft() {
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    let (wake, received) = mpsc::channel();
    let bridge = crate::bridge::Bridge::with_ports(
        Arc::new(UnusedEngine),
        Arc::new(EmptyRepository),
        Arc::new(move || {
            let _ = wake.send(());
        }),
    )
    .expect("test bridge");
    let path = std::env::temp_dir().join(format!("rune-navigation-qa-{}.pgn", std::process::id()));
    std::fs::write(&path, "1. e4 e5 *").expect("write test PGN");
    let mut state = State::fixture();
    state.bridge = Some(bridge);
    state.select_import_source(ImportSource::File);
    state.drop_pgn_files(std::slice::from_ref(&path));
    assert!(state.file_loading);
    state.navigate(Page::Settings);
    state.pgn.replace_text("1. d4 d5 *");
    received
        .recv_timeout(Duration::from_secs(5))
        .expect("file read wakes UI");
    state.poll_bridge();
    std::fs::remove_file(path).expect("remove test PGN");
    assert_eq!(
        (state.page, state.pgn.text.as_str()),
        (Page::Settings, "1. d4 d5 *"),
        "late import must preserve both the current screen and the newer draft"
    );
}
