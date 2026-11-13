//! EngineRoom's experimental native frontend, composed with Rune and OpenCode's kit.
mod audio;
mod avatar;
mod bridge;
mod components;
mod desktop_paths;
mod engine_probe;
mod file_dialog;
mod fixtures;
mod game;
mod layout;
mod preferences;
mod review_data;
mod review_session;
mod state;
mod theme;
mod variation;

use rune::{AppSpec, Cx, El};
use state::{Appearance, ImportSource, Page, Profile, SettingsSection, State};
fn view(s: &State, cx: &mut Cx<State>) -> El {
    cx.reduced_motion(s.preferences.reduced_motion);
    let k = theme::kit(s.preferences.appearance);
    let mut layers = Vec::new();
    let shell_size = cx.layout_size("shell").unwrap_or([1220.0, 900.0]);
    let review_size = [
        shell_size[0]
            - if s.sidebar.is_open() {
                s.sidebar.width()
            } else {
                36.0
            }
            - 16.0,
        shell_size[1] - 16.0,
    ];
    let layout = layout::Layout::new(if s.page == Page::Review {
        review_size
    } else {
        cx.layout_size("page-body").unwrap_or([984.0, 836.0])
    })
    .with_inspector_width(s.review_sidebar.width());
    let content = match s.page {
        Page::Home => components::home::view(s, k, layout),
        Page::Configure => components::home::configure(s, k, layout),
        Page::Review => components::review::view(s, k, layout, &mut layers),
        Page::History => components::history::view(s, k, layout),
        Page::Settings => components::settings::view(s, k, layout, &mut layers),
    };
    let trailing = (s.page == Page::Review
        && s.review_sidebar.is_open()
        && !layout.stacked_review())
    .then(|| {
        components::review::docked_sidebar(
            s,
            k,
            if cfg!(target_os = "macos") && cx.titlebar_inset() > 0.0 {
                40.0
            } else {
                8.0
            },
        )
    });
    rune_kit::root(
        components::shell::view(s, cx, content.key("page-body").fill(k.c.page), trailing, k)
            .on_key(review_shortcut),
        layers,
    )
    .on_drop_files(|s: &mut State, files, _| {
        if s.page == Page::Home && s.import_source == ImportSource::File {
            s.file_drag_over = false;
            s.drop_pgn_files(files);
        }
    })
}
fn review_shortcut(s: &mut State, key: &rune::KeyEvent, tx: &mut rune::Tx) -> bool {
    if s.page != Page::Review
        || s.analysis_settings_open
        || s.promotion_pending.is_some()
        || key.mods.ctrl
        || key.mods.alt
        || key.mods.logo
        || matches!(tx.focused(), Some("review-sidebar-grip" | "sidebar-grip"))
    {
        return false;
    }
    match &key.key {
        rune::Key::Right if key.mods.shift => s.next_critical(),
        rune::Key::Left if key.mods.shift => s.previous_critical(),
        rune::Key::Right => s.next(),
        rune::Key::Left => s.previous(),
        rune::Key::Home if !key.mods.shift => s.first(),
        rune::Key::End if !key.mods.shift => s.last(),
        rune::Key::Text(text) if !key.mods.shift && text.eq_ignore_ascii_case("f") => {
            if !tx.key_repeat() {
                s.flipped = !s.flipped;
            }
        }
        _ => return false,
    }
    reveal_move(s, tx);
    true
}

fn reveal_move(s: &State, tx: &mut rune::Tx) {
    let key = match s.visible_variation_cursor() {
        Some((i, cursor)) if cursor > 0 => {
            format!("variation-{i}-move-{cursor}")
        }
        Some((i, _)) => format!("branches-{}", s.variations[i].root_ply),
        None if s.ply > 0 => format!("move-{}", s.ply),
        None => "move-list".to_string(),
    };
    tx.scroll_into_view(key);
}

fn initial_state() -> State {
    if std::env::args_os().any(|arg| {
        matches!(
            arg.to_str(),
            Some("--snap" | "--tree" | "--scene" | "--script")
        )
    }) {
        State::fixture()
    } else {
        State::default()
    }
}
fn spec() -> AppSpec<State> {
    AppSpec::<State> {
        min_size: Some((720.0, 560.0)),
        fonts: theme::FONTS,
        families: theme::FAMILIES,
        scenes: &[
            ("sessions", |s| *s = fixtures::workspace_scene()),
            ("home-file", |s| s.import_source = ImportSource::File),
            ("home-chesscom", |s| {
                s.import_source = ImportSource::ChessCom
            }),
            ("home-chesscom-results", |s| {
                use engineroom_backend::imports::{Player, RemoteGame};
                s.import_source = ImportSource::ChessCom;
                s.chess_username.replace_text("jogador");
                s.chess_games = (1..=20)
                    .map(|i| RemoteGame {
                        url: String::new(),
                        end_time: 0,
                        time_class: Some(if i % 2 == 0 { "rapid" } else { "blitz" }.into()),
                        white: Player {
                            username: "jogador".into(),
                            rating: Some(1500),
                        },
                        black: Player {
                            username: format!("Oponente{i}"),
                            rating: Some(1450),
                        },
                        result: "1-0".into(),
                        pgn: "[UTCDate \"2026.11.08\"]\n\n1. e4 e5 *".into(),
                    })
                    .collect();
                s.chess_selected = Some(0);
            }),
            ("home-valid", |s| s.load_fixture()),
            ("home-invalid", |s| {
                s.pgn.replace_text("1. e4 e5 2. Invalid")
            }),
            ("home-light", |s| {
                s.preferences.appearance = Appearance::Light
            }),
            ("configure-deep", |s| {
                s.load_fixture();
                s.prepare();
                s.profile = Profile::Deep;
            }),
            ("review", |s| {
                *s = State::fixture();
                s.page = Page::Review
            }),
            ("review-negative", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.go_to(12);
            }),
            ("review-mate", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.real_result.as_mut().expect("fixture result").positions[0].cp = 99_997;
            }),
            ("review-flipped", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.flipped = true;
            }),
            ("review-preliminary", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.real_result.as_mut().expect("fixture result").positions[0].search =
                    Some(engineroom_backend::review::types::Search {
                        purpose: "playback".into(),
                        movetime_ms: 500,
                        multipv: 1,
                    });
            }),
            ("review-loading", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.real_result = None;
                s.real_progress = Some((5, 17));
            }),
            ("review-variation-feedback", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.click_square(52);
                s.click_square(36);
                if let Some(i) = s.active_variation {
                    s.variations[i].classifications[0] =
                        Some(engineroom_backend::review::types::Classification::Bom);
                }
            }),
            ("review-avatars", |s| {
                *s = State::fixture();
                s.fixture_avatars();
                s.game.white = "Jogador de brancas com nome muito longo".into();
                s.page = Page::Review;
            }),
            ("review-light", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.preferences.appearance = Appearance::Light;
            }),
            ("review-light-variation", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.preferences.appearance = Appearance::Light;
                s.go_to(8);
                s.explore_candidate();
            }),
            ("review-white-variation", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.go_to(7);
                s.explore_candidate();
            }),
            ("review-branches", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                for square in [52, 36, 12, 28] {
                    s.click_square(square);
                }
                s.previous();
                for square in [10, 26] {
                    s.click_square(square);
                }
            }),
            ("review-promotion", |s| {
                *s = State::fixture();
                s.game = game::parse(
                    "[SetUp \"1\"]\n[FEN \"7k/P7/8/8/8/8/4K3/8 w - - 0 1\"]\n\n1. a8=Q+ *",
                )
                .expect("promotion fixture");
                s.go_to(0);
                s.page = Page::Review;
                s.click_square(8);
                s.click_square(0);
            }),
            ("opening", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.ply = s.game.moves.len();
            }),
            ("summary", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.ply = s.game.moves.len();
                s.review_tab = state::ReviewTab::Summary;
            }),
            ("demo-analysis", |s| {
                *s = State::fixture();
                s.compact_panel = true;
                s.page = Page::Review;
                s.go_to(8);
                s.review_tab = state::ReviewTab::Analysis;
            }),
            ("demo-summary", |s| {
                *s = State::fixture();
                s.compact_panel = true;
                s.page = Page::Review;
                s.go_to(11);
                s.review_tab = state::ReviewTab::Summary;
            }),
            ("demo-collapsed", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.go_to(11);
                s.sidebar.set_open(false);
            }),
            ("demo-progress", |s| {
                *s = State::fixture();
                s.compact_panel = true;
                s.page = Page::Review;
                s.real_result = None;
                s.real_progress = Some((10, 17));
            }),
            ("demo-error", |s| {
                *s = State::fixture();
                s.compact_panel = true;
                s.page = Page::Review;
                s.real_result = None;
                s.bridge_error = Some("É necessário tentar novamente.".into());
            }),
            ("preview", |s| {
                *s = State::fixture();
                s.page = Page::Review;
                s.real_result = None;
            }),
            ("home-search-error", |s| {
                s.import_source = ImportSource::ChessCom;
                s.chess_username.replace_text("jogador");
                s.chess_searched = true;
                s.import_error = Some("Não foi possível buscar partidas.".into());
            }),
            ("history-loading", |s| {
                s.page = Page::History;
                s.history_loading = true;
            }),
            ("history-error", |s| {
                s.page = Page::History;
                s.history_error = Some("Não foi possível carregar o histórico.".into());
            }),
            ("history-no-results", |s| {
                s.page = Page::History;
                s.history_filter.replace_text("sem resultados");
            }),
            ("history-delete-pending", |s| {
                s.page = Page::History;
                s.history.push(engineroom_backend::db::games::GameSummary {
                    id: 1,
                    white: "José Silva".into(),
                    black: "Ana Costa".into(),
                    result: "1-0".into(),
                    plies: 40,
                    engine_tier: "custom".into(),
                    mode: engineroom_backend::db::mode::Mode::Depth,
                    analysis_kind: "manual".into(),
                    depth: 20,
                    multipv: 2,
                    accuracy_white: 80.0,
                    accuracy_black: 70.0,
                    created_at: "2026-11-10".into(),
                });
                s.history_total = 1;
                s.history_confirm_delete = Some(1);
                s.history_delete_pending = true;
            }),
            ("settings-probe-running", |s| {
                s.open_settings();
                s.section = SettingsSection::Analysis;
                s.engine_probe = engine_probe::Status::Running;
            }),
            ("settings-probe-success", |s| {
                s.open_settings();
                s.section = SettingsSection::Analysis;
                s.engine_probe = engine_probe::Status::Success(Some("Stockfish 19".into()));
            }),
            ("history", |s| s.page = Page::History),
            ("history-search-delete", |s| {
                s.page = Page::History;
                s.history.push(engineroom_backend::db::games::GameSummary {
                    id: 1,
                    white: "José Silva".into(),
                    black: "Ana Costa".into(),
                    result: "1-0".into(),
                    plies: 40,
                    engine_tier: "custom".into(),
                    mode: engineroom_backend::db::mode::Mode::Depth,
                    analysis_kind: "manual".into(),
                    depth: 20,
                    multipv: 2,
                    accuracy_white: 80.0,
                    accuracy_black: 70.0,
                    created_at: "2026-11-10".into(),
                });
                s.history_total = 1;
                s.history_filter.replace_text("jose");
                s.request_history_delete(1);
                s.history_delete_error = Some("Não foi possível excluir a revisão.".into());
            }),
            ("settings-probe-error", |s| {
                s.open_settings();
                s.section = SettingsSection::Analysis;
                s.engine_probe =
                    engine_probe::Status::Error("Não foi possível iniciar o Stockfish.".into());
            }),
            ("configure", |s| {
                s.load_fixture();
                s.prepare();
            }),
            ("manual", |s| {
                s.load_fixture();
                s.prepare();
                s.profile = state::Profile::Manual;
            }),
            ("settings", |s| s.open_settings()),
            ("settings-light", |s| {
                s.open_settings();
                s.preferences.appearance = Appearance::Light;
            }),
            ("settings-analysis", |s| {
                s.open_settings();
                s.section = SettingsSection::Analysis;
            }),
            ("settings-storage", |s| {
                s.open_settings();
                s.section = SettingsSection::Storage;
            }),
        ],
        setup: Some(|s, wake| {
            if !cfg!(test)
                && !std::env::args_os().any(|arg| {
                    matches!(
                        arg.to_str(),
                        Some("--snap" | "--tree" | "--scene" | "--script")
                    )
                })
            {
                s.setup_avatars(wake.clone());
                s.setup_bridge(wake.clone());
                s.setup_preferences(wake.clone());
                s.setup_file_dialog(wake);
            }
        }),
        poll: Some(|s, now, tx| {
            let before = (
                s.active_variation,
                s.active_variation.map(|i| s.variations[i].cursor),
            );
            let changed = s.tick(now) | s.poll_bridge();
            if s.history_focus_after_delete {
                s.history_focus_after_delete = false;
                tx.focus("history-search", true);
            }
            let after = (
                s.active_variation,
                s.active_variation.map(|i| s.variations[i].cursor),
            );
            if before != after {
                reveal_move(s, tx);
            }
            if s.playing {
                tx.wake_after(std::time::Duration::from_millis(50));
            }
            changed
        }),
        on_key: Some(|s, key, tx| {
            if s.page == Page::History
                && s.history_confirm_delete.is_some()
                && matches!(key.key, rune::Key::Escape)
            {
                s.cancel_history_delete();
                return true;
            }
            // Control is literal on every platform; cmd is also set with it on Linux/Windows.
            if s.page == Page::Review
                && key.mods.ctrl
                && key.mods.alt
                && !key.mods.shift
                && !key.mods.logo
                && (key.is_command('b')
                    || matches!(&key.key, rune::Key::Text(text) if text.eq_ignore_ascii_case("b"))
                    || tx.key_base() == Some('b'))
            {
                if !s.analysis_settings_open && !tx.key_repeat() {
                    s.review_sidebar.toggle();
                }
                return true;
            }
            if (key.is_command('b') && !key.mods.shift && !key.mods.alt)
                || rune_kit::shell::is_sidebar_toggle(key)
            {
                s.sidebar.toggle();
                return true;
            }
            review_shortcut(s, key, tx)
        }),
        ..AppSpec::new("EngineRoom · Rune", (1220.0, 900.0), initial_state, view)
    }
}
fn main() {
    rune::run(spec());
}
#[cfg(test)]
mod perf_headless;
#[cfg(test)]
mod tests;
