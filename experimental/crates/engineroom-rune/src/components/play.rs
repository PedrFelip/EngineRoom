//! Games use the review workspace, move list, player rows, and inspector sizing.
use super::{review, ui};
use crate::{
    layout::Layout,
    play::{MAX_ELO, MIN_ELO, Side},
    state::State,
};
use rune::{El, ScrollSpec, col, row};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, SegmentedControl};
use shakmaty::Position;

fn setup(s: &State, k: &Kit) -> El {
    let rating = ui::slider::<State>(
        "play-rating",
        "Rating do Stockfish",
        (s.play.rating - MIN_ELO) as f32 / (MAX_ELO - MIN_ELO) as f32,
        |s| (s.play.rating - MIN_ELO) as f32 / (MAX_ELO - MIN_ELO) as f32,
        |s, value| {
            s.play.rating = (MIN_ELO
                + ((MAX_ELO - MIN_ELO) as f32 * value.clamp(0.0, 1.0)).round() as u32)
                .clamp(MIN_ELO, MAX_ELO)
        },
        k,
    )
    .range_value(
        f64::from(s.play.rating),
        f64::from(MIN_ELO),
        f64::from(MAX_ELO),
    )
    .range_step(10.0)
    .a11y_value(format!("{} Elo", s.play.rating))
    .on_step(|s: &mut State, steps, _| {
        s.play.rating = (i64::from(s.play.rating) + i64::from(steps) * 10)
            .clamp(i64::from(MIN_ELO), i64::from(MAX_ELO)) as u32;
    });
    let colors = SegmentedControl::new(
        &["Brancas", "Pretas", "Aleatório"],
        Some(match s.play.side {
            Side::White => 0,
            Side::Black => 1,
            Side::Random => 2,
        }),
    )
    .label("Sua cor")
    .width(None)
    .on_pick(|s: &mut State, i, _| {
        s.play.side = match i {
            0 => Side::White,
            1 => Side::Black,
            _ => Side::Random,
        }
    })
    .build(k)
    .key("play-color");
    col().key("play-setup-scroll").grow(1.0).min_h(0.0).w_full()
        .scroll(ScrollSpec { stick_bottom: false }).padding(24.0).items_center()
        .child(col().key("play-setup-card").w_full().max_w(480.0).gap(24.0).padding(24.0)
            .radius(k.radius.lg).border(1.0, k.c.border_base).fill(k.c.bg_deep)
            .child(col().gap(8.0)
                .child(ui::label("Nova partida", 22.0, k.c.text_base))
                .child(ui::label("Desafie o Stockfish no seu ritmo. Escolha a força do motor e a sua cor.", 14.0, k.c.text_muted).wrap().w_full()))
            .child(ui::rule(k))
            .child(col().gap(12.0)
                .child(ui::label("FORÇA DO MOTOR", 12.0, k.c.text_muted))
                .child(ui::label(format!("{} Elo", s.play.rating), 24.0, k.c.text_base).key("play-rating-label"))
                .child(rating)
                .child(ui::label(format!("{MIN_ELO}–{MAX_ELO} Elo · estimativa do Stockfish"), 12.0, k.c.text_muted).wrap().w_full()))
            .child(col().gap(10.0).child(ui::label("JOGAR DE", 12.0, k.c.text_muted)).child(colors))
            .child(Button::new("Iniciar partida").variant(ButtonVariant::Contrast).control_size(ControlSize::Large).build(k)
                .key("play-start").on_press_tx(|s: &mut State, tx| {
                    s.start_play();
                    if let Some(key) = s.play.active { tx.scroll_into_view(format!("play-session-{key}")); }
                }))
            .child(ui::label("Sem relógio · análise disponível ao terminar", 12.0, k.c.text_muted).wrap().w_full()))
}
fn status(s: &State) -> String {
    let Some(game) = &s.play.game else {
        return String::new();
    };
    if let Some(result) = game.result {
        format!("{} · {result}", game.reason.unwrap_or("Partida encerrada"))
    } else if s.play.thinking {
        "Stockfish está pensando…".into()
    } else if game.human_turn() {
        if game.position().is_check() {
            "Sua vez · xeque".into()
        } else {
            "Sua vez".into()
        }
    } else {
        "Vez do Stockfish".into()
    }
}
fn actions(s: &State, k: &Kit) -> El {
    let mut actions = col().gap(8.0).w_full();
    if let Some(game) = &s.play.game {
        if let Some(error) = &s.play.error
            && game.result.is_none()
        {
            actions = actions
                .child(ui::label(error, 13.0, k.c.text_muted).wrap().w_full())
                .child(
                    Button::new("Tentar novamente")
                        .build(k)
                        .key("play-retry")
                        .on_press(|s: &mut State| s.retry_play()),
                );
        }
        if game.result.is_some() {
            if !game.moves.is_empty() {
                actions = actions.child(
                    Button::new("Analisar partida")
                        .variant(ButtonVariant::Contrast)
                        .build(k)
                        .key("play-analyze")
                        .on_press(|s: &mut State| s.analyze_play()),
                );
            }
            actions = actions.child(
                Button::new("Nova partida")
                    .build(k)
                    .key("play-new")
                    .on_press(|s: &mut State| s.open_play_setup()),
            );
        } else {
            if s.play.cursor < game.moves.len() {
                actions = actions.child(
                    Button::new("Voltar à posição atual")
                        .build(k)
                        .key("play-current")
                        .on_press(|s: &mut State| s.last()),
                );
            }
            actions = actions.child(
                Button::new("Desistir")
                    .build(k)
                    .key("play-resign")
                    .on_press(|s: &mut State| s.resign_play()),
            );
        }
    }
    actions
}
pub(super) fn inspector(s: &State, k: &Kit) -> El {
    col()
        .key("review-inspector")
        .role("region")
        .label("Lances da partida")
        .min_h(0.0)
        .padding(16.0)
        .gap(12.0)
        .fill(k.c.bg_deep)
        .child(
            row()
                .center_y()
                .min_h(36.0)
                .child(ui::label("Lances", 16.0, k.c.text_base).grow(1.0))
                .child(review::panel_toggle(s, k)),
        )
        .child(
            ui::label(status(s), 13.0, k.c.text_muted)
                .wrap()
                .w_full()
                .key("play-status"),
        )
        .child(ui::rule(k))
        .child(
            col()
                .key("review-panel-scroll")
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .child(if s.displayed_game().moves.is_empty() {
                    ui::label(
                        "Os lances da partida aparecerão aqui.",
                        13.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full()
                } else {
                    review::move_list(s, k)
                }),
        )
        .child(ui::rule(k))
        .child(actions(s, k))
}
pub fn view(s: &State, k: &Kit, layout: Layout, layers: &mut Vec<El>) -> El {
    if s.play.game.is_none() {
        return setup(s, k);
    }
    review::promotion_dialog(s, k, layers);
    let game = s.displayed_game();
    let compact = layout.stacked_review();
    let header = row()
        .min_h(layout.header_height())
        .px(16.0)
        .gap(8.0)
        .center_y()
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(
                    ui::label(
                        format!("{} × {}", game.white, game.black),
                        18.0,
                        k.c.text_base,
                    )
                    .truncate()
                    .w_full(),
                )
                .child(
                    ui::label(status(s), 12.0, k.c.text_muted)
                        .truncate()
                        .w_full(),
                ),
        )
        .child(
            Button::new("Nova partida")
                .build(k)
                .key("play-add")
                .on_press(|s: &mut State| s.open_play_setup()),
        )
        .children((!s.review_sidebar.is_open()).then(|| review::panel_toggle(s, k)));
    let size = if s.review_sidebar.is_open() {
        layout.board_size()
    } else {
        layout.focus_board_size()
    };
    let size = if compact {
        (size - 32.0).max(240.0)
    } else {
        size
    };
    let board = col()
        .key("board-scroll")
        .grow(1.0)
        .min_h(0.0)
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(review::workspace(s, k, size, layout.compact_height()));
    let board = if compact || !s.review_sidebar.is_open() {
        col()
            .grow(1.0)
            .min_h(0.0)
            .child(board)
            .child(actions(s, k).padding(8.0))
    } else {
        board
    };
    col()
        .key("play-page")
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .child(header)
        .children((compact && s.review_sidebar.is_open()).then(|| {
            SegmentedControl::new(&["Tabuleiro", "Lances"], Some(usize::from(s.compact_panel)))
                .label("Painéis da partida")
                .width(None)
                .on_pick(|s: &mut State, i, _| s.compact_panel = i == 1)
                .build(k)
                .key("play-panels")
        }))
        .child(ui::rule(k))
        .child(
            if compact && s.compact_panel && s.review_sidebar.is_open() {
                inspector(s, k).grow(1.0).min_h(0.0)
            } else {
                board
            },
        )
}
