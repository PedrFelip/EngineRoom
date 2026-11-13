//! Opt-in process benchmarks: actual Rune runtime, CPU-side frames, no GPU.
use super::*;
use rune::{Env, Key, KeyEvent, Runtime};

const GAME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../perf/data/immortal.pgn"
));

fn runtime() -> Runtime<State> {
    let mut rt = rune::App::new(spec()).headless_runtime(
        (1220.0, 900.0),
        Env {
            system_dark: true,
            scale: 1.0,
        },
    );
    rt.state.preferences.sound = false;
    rt.invalidate();
    rt.frame(0.0);
    rt
}

fn tap(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..10 {
        rt.frame(1.0 / 60.0);
    }
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..10 {
        rt.frame(1.0 / 60.0);
    }
}

fn key(rt: &mut Runtime<State>, key: Key) {
    rt.key(KeyEvent {
        key,
        mods: Default::default(),
    });
    rt.frame(1.0 / 60.0);
}

fn open_real_game(rt: &mut Runtime<State>) {
    tap(rt, "pgn-editor/scroll/input");
    key(rt, Key::Text(GAME.into()));
    tap(rt, "continue");
    assert_eq!(rt.state.page, Page::Configure);
    assert_eq!(rt.state.game.moves.len(), 45);
    assert_eq!(rt.state.game.white, "Adolf Anderssen");
    tap(rt, "start-review");
    assert_eq!(rt.state.page, Page::Review);
}

#[test]
#[ignore = "opt-in Strobe workload"]
fn import_and_open_real_game() {
    for _ in 0..4 {
        let mut rt = runtime();
        open_real_game(&mut rt);
    }
}

#[test]
#[ignore = "opt-in Strobe workload"]
fn navigate_real_game() {
    let mut rt = runtime();
    open_real_game(&mut rt);
    for _ in 0..2 {
        for ply in 1..=45 {
            key(&mut rt, Key::Right);
            assert_eq!(rt.state.ply, ply);
        }
        key(&mut rt, Key::Home);
        assert_eq!(rt.state.ply, 0);
    }
}

#[test]
#[ignore = "opt-in Strobe workload"]
fn rebuild_review_frames() {
    let mut rt = runtime();
    open_real_game(&mut rt);
    for _ in 0..100 {
        rt.invalidate();
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.game.moves.len(), 45);
}

#[test]
#[ignore = "interactive Strobe driver"]
fn serve() {
    use std::io::{self, BufRead, Write};
    let mut rt = runtime();
    open_real_game(&mut rt);
    println!("STROBE_READY");
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        match line.unwrap().as_str() {
            "import" => {
                let mut fresh = runtime();
                open_real_game(&mut fresh);
            }
            "navigate" => {
                for ply in 1..=45 {
                    key(&mut rt, Key::Right);
                    assert_eq!(rt.state.ply, ply);
                }
            }
            "reset" => {
                key(&mut rt, Key::Home);
                assert_eq!(rt.state.ply, 0);
            }
            "rebuild" => {
                for _ in 0..100 {
                    rt.invalidate();
                    rt.frame(1.0 / 60.0);
                }
                assert_eq!(rt.state.game.moves.len(), 45);
            }
            "quit" => break,
            command => panic!("unknown command: {command}"),
        }
        println!("STROBE_DONE");
        io::stdout().flush().unwrap();
    }
}
