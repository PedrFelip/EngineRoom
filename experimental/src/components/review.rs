//! Full review presentation with opt-in fixture data and legal board exploration.
use super::{board, review_analysis, review_feedback, review_graph, review_summary, ui};
use crate::{
    layout::Layout,
    review_data,
    state::{LearningSide, MockStatus, Page, ReviewTab, State},
};
use rune::{El, ScrollSpec, col, row};
use rune_kit::{Kit, Progress, SegmentedControl, Switch};

fn playback(s: &State, k: &Kit) -> El {
    let counter = match s.active_variation {
        Some(i) => format!(
            "V{} {}/{}",
            i + 1,
            s.variations[i].cursor,
            s.variations[i].moves.len()
        ),
        None => format!("{:02} / {}", s.ply, s.game.moves.len()),
    };
    row()
        .key("playback")
        .gap(6.0)
        .center_y()
        .child(
            ui::button("first", "|‹", k)
                .label("Primeiro lance")
                .on_press(State::first),
        )
        .child(
            ui::button("previous", "‹", k)
                .label("Lance anterior")
                .on_press(State::previous),
        )
        .child(ui::mono(counter, 12.0, k.c.text_muted).w(76.0).center())
        .child(
            ui::button("next", "›", k)
                .label("Próximo lance")
                .on_press(State::next),
        )
        .child(
            ui::button("last", "›|", k)
                .label("Último lance")
                .on_press(State::last),
        )
        .child(ui::button("flip", "Girar", k).on_press(|s: &mut State| s.flipped = !s.flipped))
}
fn workspace(s: &State, k: &Kit, size: f32) -> El {
    let (top, bottom) = if s.flipped {
        ((&s.game.white, "Brancas"), (&s.game.black, "Pretas"))
    } else {
        ((&s.game.black, "Pretas"), (&s.game.white, "Brancas"))
    };
    let mut content = col()
        .key("board-workspace")
        .w_full()
        .items_center()
        .gap(8.0)
        .py(12.0)
        .child(board::player(top.0, top.1, k, size + 36.0))
        .child(
            row()
                .gap(10.0)
                .items_start()
                .child(review_graph::meter(s, k, size))
                .child(board::view(s, k, size)),
        )
        .child(board::player(bottom.0, bottom.1, k, size + 36.0))
        .child(playback(s, k));
    if let Some(i) = s.active_variation {
        content = content
            .child(
                row()
                    .gap(8.0)
                    .flex_wrap()
                    .center_y()
                    .child(
                        ui::button("return-mainline", "↩ Voltar à partida", k)
                            .on_press(|s: &mut State| s.go_to(s.ply)),
                    )
                    .child(
                        ui::button(
                            "pause-variation",
                            if s.playing { "Pausar" } else { "Reproduzir" },
                            k,
                        )
                        .on_press_tx(|s: &mut State, tx| {
                            s.playing = !s.playing;
                            s.next_step = s.clock + 0.65;
                            tx.wake_after(std::time::Duration::from_millis(50));
                        }),
                    ),
            )
            .child(ui::label(
                format!(
                    "Variação {} · a partir do meio-lance {}",
                    i + 1,
                    s.variations[i].root_ply
                ),
                12.0,
                k.c.text_muted,
            ));
    } else {
        content = content.child(ui::label(
            "← → lances · Shift+← → momentos críticos",
            12.0,
            k.c.text_muted,
        ));
    }
    content
}
fn learning(s: &State, k: &Kit) -> El {
    let selected = match s.learning_side {
        LearningSide::All => 0,
        LearningSide::White => 1,
        LearningSide::Black => 2,
    };
    let filter = SegmentedControl::new(&["Todas", "Brancas", "Pretas"], Some(selected))
        .label("Filtrar momentos críticos")
        .width(None)
        .on_pick(|s: &mut State, i, _| {
            s.learning_side = match i {
                0 => LearningSide::All,
                1 => LearningSide::White,
                _ => LearningSide::Black,
            }
        })
        .build(k)
        .key("learning-filter");
    let mut items = col().gap(8.0).child(filter);
    let critical: Vec<usize> = (1..=s.game.moves.len())
        .filter(|&p| s.critical_matches(p))
        .collect();
    for ply in &critical {
        let ply = *ply;
        let class = review_data::classification(ply);
        items = items.child(
            ui::button(
                &format!("critical-{ply}"),
                &format!(
                    "{}. {} · {}",
                    ply.div_ceil(2),
                    s.game.moves[ply - 1],
                    class.label()
                ),
                k,
            )
            .w_full()
            .on_press(move |s: &mut State| s.go_to(ply)),
        );
    }
    if critical.is_empty() {
        items = items.child(ui::label(
            "Nenhum momento para este filtro.",
            12.0,
            k.c.text_muted,
        ));
    }
    ui::section("Momentos para aprender · mock", items, k)
}
fn move_list(s: &State, k: &Kit) -> El {
    let mut list = col().key("move-list").w_full().gap(4.0);
    for (index, pair) in s.game.moves.chunks(2).enumerate() {
        let mut line = row()
            .gap(6.0)
            .center_y()
            .child(ui::mono(format!("{}.", index + 1), 12.0, k.c.text_muted).w(24.0));
        for (side, notation) in pair.iter().enumerate() {
            let ply = index * 2 + side + 1;
            let mut cell = row().grow(1.0).min_w(0.0).gap(5.0).center_y().child(
                ui::button(&format!("move-{ply}"), notation, k)
                    .grow(1.0)
                    .min_w(0.0)
                    .fill(if s.ply == ply && s.active_variation.is_none() {
                        k.c.layer_01
                    } else {
                        k.c.bg_base
                    })
                    .on_press(move |s: &mut State| s.go_to(ply)),
            );
            if s.is_mock() && s.preferences.feedback {
                cell = cell.child(review_feedback::badge(review_data::classification(ply)));
            }
            line = line.child(cell);
        }
        list = list.child(line);
    }
    list
}
fn moves_panel(s: &State, k: &Kit) -> El {
    let mut content = col().gap(20.0).child(move_list(s, k));
    if s.is_mock() {
        content = content.child(learning(s, k));
    }
    if !s.variations.is_empty() {
        let mut branches = col().gap(8.0);
        for (i, v) in s.variations.iter().enumerate() {
            branches = branches.child(
                ui::button(
                    &format!("variation-{i}"),
                    &format!("V{} · {}: {}", i + 1, v.root_ply, v.san.join(" ")),
                    k,
                )
                .w_full()
                .on_press(move |s: &mut State| {
                    s.active_variation = Some(i);
                    s.playing = false;
                }),
            );
        }
        content = content.child(ui::section("Variações locais", branches, k));
    }
    content
}
fn status(s: &State, k: &Kit) -> El {
    if !s.is_mock() {
        return ui::label(
            "Prévia local · ative Demo para ver feedbacks fictícios.",
            12.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full();
    }
    match s.mock_status {
        MockStatus::Ready => review_feedback::current(s, k).w_full(),
        MockStatus::Running => col()
            .gap(8.0)
            .child(ui::label(
                format!("Analisando · {:.0}% · simulação", s.mock_progress * 100.0),
                13.0,
                k.c.text_base,
            ))
            .child(
                Progress::new(Some(s.mock_progress))
                    .build(k)
                    .key("mock-progress"),
            )
            .child(
                ui::button("cancel-analysis", "Cancelar simulação", k)
                    .on_press(|s: &mut State| s.mock_status = MockStatus::Cancelled),
            ),
        MockStatus::Failed => col()
            .key("mock-error")
            .gap(8.0)
            .role("alert")
            .child(ui::label("Falha simulada na análise", 14.0, k.c.danger.fg))
            .child(
                ui::button("retry-analysis", "Tentar novamente", k).on_press(|s: &mut State| {
                    s.mock_status = MockStatus::Running;
                    s.mock_progress = 0.0;
                }),
            ),
        MockStatus::Cancelled => ui::label(
            "Simulação cancelada · resultados fictícios preservados",
            12.0,
            k.c.text_muted,
        )
        .key("mock-cancelled")
        .wrap()
        .w_full(),
    }
}
fn inspector(s: &State, k: &Kit) -> El {
    let selected = match s.review_tab {
        ReviewTab::Moves => 0,
        ReviewTab::Analysis => 1,
        ReviewTab::Summary => 2,
    };
    let tabs = SegmentedControl::new(&["Lances", "Análise", "Resumo"], Some(selected))
        .label("Painéis da revisão")
        .width(None)
        .on_pick(|s: &mut State, i, _| {
            s.review_tab = match i {
                0 => ReviewTab::Moves,
                1 => ReviewTab::Analysis,
                _ => ReviewTab::Summary,
            };
            s.playing = false;
        })
        .build(k)
        .key("review-tabs");
    let content = match s.review_tab {
        ReviewTab::Moves => moves_panel(s, k),
        ReviewTab::Analysis => review_analysis::view(s, k),
        ReviewTab::Summary => review_summary::view(s, k),
    };
    col()
        .key("review-inspector")
        .min_h(0.0)
        .padding(16.0)
        .gap(12.0)
        .fill(k.c.bg_base)
        .child(tabs)
        .child(status(s, k))
        .child(ui::rule(k))
        .child(
            col()
                .key("review-panel-scroll")
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(content),
        )
        .child(
            ui::label(
                if s.is_mock() {
                    "DEMO · scores e classificações fictícios"
                } else {
                    "PGN validado · motor não conectado"
                },
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
}
fn toolbar(s: &State, k: &Kit) -> El {
    let demo = Switch::new(s.is_mock())
        .label("Demo · dados fictícios")
        .on_toggle(review_analysis::toggle_mode)
        .build(k)
        .key("mock-toggle");
    let engine = Switch::new(s.preferences.live_engine)
        .label("Análise")
        .on_toggle(|s: &mut State| s.preferences.live_engine = !s.preferences.live_engine)
        .build(k)
        .key("review-engine-toggle");
    let mut bar = row()
        .flex_wrap()
        .gap(16.0)
        .center_y()
        .px(16.0)
        .py(8.0)
        .child(demo)
        .child(engine);
    if s.is_mock() {
        let label = match s.mock_status {
            MockStatus::Ready => "Cenário: pronto",
            MockStatus::Running => "Cenário: analisando",
            MockStatus::Failed => "Cenário: erro",
            MockStatus::Cancelled => "Cenário: cancelado",
        };
        bar = bar.child(
            ui::button("mock-scenario", label, k).on_press_tx(|s: &mut State, tx| {
                s.mock_status = match s.mock_status {
                    MockStatus::Ready => MockStatus::Running,
                    MockStatus::Running => MockStatus::Failed,
                    MockStatus::Failed => MockStatus::Cancelled,
                    MockStatus::Cancelled => MockStatus::Ready,
                };
                s.mock_progress = 0.0;
                tx.wake_after(std::time::Duration::from_millis(50));
            }),
        );
    }
    bar
}
pub fn view(s: &State, k: &Kit, layout: Layout, layers: &mut Vec<El>) -> El {
    review_analysis::dialog(s, k, layers);
    let mut header = row()
        .min_h(56.0)
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
                        format!("{} × {}", s.game.white, s.game.black),
                        18.0,
                        k.c.text_base,
                    )
                    .truncate()
                    .w_full(),
                )
                .child(
                    ui::label(
                        format!("{} · {} meios-lances", s.game.event, s.game.moves.len()),
                        12.0,
                        k.c.text_muted,
                    )
                    .truncate()
                    .w_full(),
                ),
        )
        .child(
            ui::button(
                "focus-board",
                if s.inspector_visible {
                    "Foco"
                } else {
                    "Painéis"
                },
                k,
            )
            .label("Alternar foco no tabuleiro")
            .on_press(|s: &mut State| s.inspector_visible = !s.inspector_visible),
        )
        .child(
            ui::button("review-settings", "⋯", k)
                .label("Configurar análise")
                .on_press(|s: &mut State| s.analysis_settings_open = true),
        )
        .child(
            ui::button("review-back", "Voltar", k).on_press(|s: &mut State| s.navigate(Page::Home)),
        );
    if layout.stacked_review() && s.inspector_visible {
        header = header.child(
            ui::button("show-analysis", "↓", k)
                .label("Ver painéis da revisão")
                .on_press_tx(|_: &mut State, tx| tx.scroll_into_view("review-inspector")),
        );
    }
    let size = if s.inspector_visible {
        layout.board_size()
    } else {
        layout.focus_board_size()
    };
    let board = workspace(s, k, size);
    let body = if !s.inspector_visible {
        col()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .key("review-scroll")
            .child(board)
    } else if layout.stacked_review() {
        col()
            .key("review-scroll")
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(
                col()
                    .w_full()
                    .child(board)
                    .child(inspector(s, k).w_full().h(560.0)),
            )
    } else {
        row()
            .grow(1.0)
            .min_h(0.0)
            .child(
                col()
                    .grow(1.0)
                    .min_w(0.0)
                    .h_full()
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .key("board-scroll")
                    .child(board),
            )
            .child(inspector(s, k).w(layout.inspector_width()).h_full())
    };
    col()
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .child(header)
        .child(toolbar(s, k))
        .child(ui::rule(k))
        .child(body)
}
