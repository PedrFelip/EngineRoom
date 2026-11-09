//! Full review presentation with deterministic fixture scenes and legal board exploration.
use super::{board, review_analysis, review_feedback, review_graph, review_summary, ui};
use crate::{
    layout::Layout,
    state::{LearningSide, ReviewTab, State},
};
use rune::{Color, Cursor, Drag, El, Icon, Phase, ScrollSpec, col, el, row};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Progress,
    SegmentedControl, Switch,
};

// Primer Octicons, MIT; attribution is bundled in assets/OCTICONS-LICENSE.txt.
static SIDEBAR_EXPAND: Icon = Icon {
    name: "sidebar-expand",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../assets/sidebar-expand-16.svg"),
        "</g>"
    ),
};
static SIDEBAR_COLLAPSE: Icon = Icon {
    name: "sidebar-collapse",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../assets/sidebar-collapse-16.svg"),
        "</g>"
    ),
};

fn panel_toggle(s: &State, k: &Kit) -> El {
    IconButton::new(if s.review_sidebar.is_open() {
        &SIDEBAR_COLLAPSE
    } else {
        &SIDEBAR_EXPAND
    })
    .variant(IconButtonVariant::GhostMuted)
    .control_size(ControlSize::ExtraLarge)
    .label(if s.review_sidebar.is_open() {
        "Ocultar painéis da revisão"
    } else {
        "Mostrar painéis da revisão"
    })
    .help(if s.review_sidebar.is_open() {
        "Ocultar painéis da revisão"
    } else {
        "Mostrar painéis da revisão"
    })
    .shortcut("Ctrl+Alt+B")
    .expanded(s.review_sidebar.is_open())
    .build(k)
    .key("focus-board")
    .on_press(|s: &mut State| s.review_sidebar.toggle())
}

pub fn docked_sidebar(s: &State, k: &Kit, top_inset: f32) -> El {
    row()
        .key("review-sidebar-pane")
        .w(s.review_sidebar.width() + 10.0)
        .h_full()
        .child(resize_grip(s))
        .child(
            inspector(s, k, false)
                .w(s.review_sidebar.width())
                .h_full()
                .pt(top_inset),
        )
}

fn resize_grip(s: &State) -> El {
    el().key("review-sidebar-grip")
        .focus_return("focus-board")
        .role("separator")
        .label("Redimensionar painéis da revisão")
        .range_value(
            s.review_sidebar.width() as f64,
            s.review_sidebar.min_width() as f64,
            s.review_sidebar.max_width() as f64,
        )
        .range_step(8.0)
        .w(10.0)
        .h_full()
        .cursor(Cursor::ResizeColumn)
        .fill(Color::TRANSPARENT)
        .help("Arraste ou use as setas para ajustar. Duplo clique restaura a largura.")
        .on_drag(|s: &mut State, d: &mut Drag, tx| match d.phase {
            Phase::Down => d.data = s.review_sidebar.width(),
            Phase::Move => {
                tx.instant();
                s.review_sidebar.set_width(d.data - d.dx);
            }
            Phase::Up => {
                if d.cancelled() {
                    s.review_sidebar.set_width(d.data);
                }
            }
        })
        .on_double_press(|s: &mut State| s.review_sidebar.reset_width())
        .on_step(|s: &mut State, steps, _| {
            s.review_sidebar
                .set_width(s.review_sidebar.width() + steps as f32 * 8.0)
        })
}

static RETURN: Icon = Icon {
    name: "return-mainline",
    view: 20.0,
    body: r#"<path d="M7 5L3 9L7 13M3 9H12C15 9 17 11 17 14V15" stroke="currentColor" stroke-linecap="square"/>"#,
};

macro_rules! octicon {
    ($name:ident, $file:literal, $label:literal) => {
        static $name: Icon = Icon {
            name: $label,
            view: 16.0,
            body: concat!(
                "<g fill=\"currentColor\" stroke=\"none\">",
                include_str!($file),
                "</g>"
            ),
        };
    };
}
octicon!(FIRST, "../../assets/move-to-start-16.svg", "move-to-start");
octicon!(PREVIOUS, "../../assets/chevron-left-16.svg", "chevron-left");
octicon!(NEXT, "../../assets/chevron-right-16.svg", "chevron-right");
octicon!(LAST, "../../assets/move-to-end-16.svg", "move-to-end");
octicon!(FLIP, "../../assets/arrow-switch-16.svg", "arrow-switch");

fn playback(s: &State, k: &Kit) -> El {
    let (position, total) = s.active_variation.map_or((s.ply, s.game.moves.len()), |i| {
        (s.variations[i].cursor, s.variations[i].moves.len())
    });
    let counter = s.active_variation.map_or_else(
        || format!("{position:02} / {total}"),
        |i| format!("V{} {position}/{total}", i + 1),
    );
    let nav = |key: &str,
               label: &str,
               shortcut: &str,
               icon: &'static Icon,
               disabled: bool,
               action: fn(&mut State)| {
        IconButton::new(icon)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Large)
            .label(label)
            .help(label)
            .shortcut(shortcut)
            .disabled(disabled)
            .on_press(action)
            .build(k)
            .key(key)
            .size(32.0, 32.0)
    };
    row()
        .key("playback")
        .role("group")
        .label("Navegação do tabuleiro")
        .gap(6.0)
        .center_y()
        .h(36.0)
        .child(
            row()
                .key("move-navigation")
                .role("group")
                .label("Navegar pelos lances")
                .gap(0.0)
                .padding(1.0)
                .center_y()
                .radius(k.radius.md)
                .border(1.0, k.c.border_base)
                .fill(k.c.bg_deep)
                .child(nav(
                    "first",
                    "Primeiro lance",
                    "Home",
                    &FIRST,
                    position == 0,
                    State::first,
                ))
                .child(nav(
                    "previous",
                    "Lance anterior",
                    "←",
                    &PREVIOUS,
                    position == 0,
                    State::previous,
                ))
                .child(
                    ui::mono(counter, 12.0, k.c.text_muted)
                        .key("move-counter")
                        .role("status")
                        .label(format!("Meio-lance {position} de {total}"))
                        .w(72.0)
                        .center(),
                )
                .child(nav(
                    "next",
                    "Próximo lance",
                    "→",
                    &NEXT,
                    position >= total,
                    State::next,
                ))
                .child(nav(
                    "last",
                    "Último lance",
                    "End",
                    &LAST,
                    position >= total,
                    State::last,
                )),
        )
        .child(
            IconButton::new(&FLIP)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Large)
                .label("Girar tabuleiro")
                .help("Girar tabuleiro")
                .shortcut("F")
                .on_press(|s: &mut State| s.flipped = !s.flipped)
                .build(k)
                .key("flip")
                .checked(s.flipped)
                .size(32.0, 32.0),
        )
}
fn workspace(s: &State, k: &Kit, size: f32, compact: bool) -> El {
    let (top, bottom) = if s.flipped {
        ((&s.game.white, "Brancas"), (&s.game.black, "Pretas"))
    } else {
        ((&s.game.black, "Pretas"), (&s.game.white, "Brancas"))
    };
    let mut content = col()
        .key("board-workspace")
        .w_full()
        .items_center()
        .gap(if compact { 6.0 } else { 8.0 })
        .py(if compact { 8.0 } else { 12.0 })
        .child(board::player(
            top.0,
            top.1,
            s.avatar(if s.flipped { 0 } else { 1 }),
            &s.game.ratings[if s.flipped { 0 } else { 1 }],
            k,
            size + 36.0,
            compact,
        ))
        .child(
            row()
                .gap(10.0)
                .items_start()
                .child(review_graph::meter(s, k, size))
                .child(board::view(s, k, size)),
        )
        .child(board::player(
            bottom.0,
            bottom.1,
            s.avatar(if s.flipped { 1 } else { 0 }),
            &s.game.ratings[if s.flipped { 1 } else { 0 }],
            k,
            size + 36.0,
            compact,
        ))
        .child(playback(s, k));
    if let Some(i) = s.active_variation {
        content = content
            .child(
                row()
                    .gap(8.0)
                    .flex_wrap()
                    .center_y()
                    .child(
                        ui::button_icon("return-mainline", "Voltar à partida", &RETURN, k)
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
    items = items.child(
        ui::label(
            if critical.is_empty() {
                "Nenhum momento crítico neste filtro".to_string()
            } else {
                format!("{} momentos críticos · escolha um lance", critical.len())
            },
            12.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full(),
    );
    let mut moves = row().gap(6.0).flex_wrap();
    for ply in critical {
        let notation = &s.game.moves[ply - 1];
        let label = format!(
            "{}{} {}",
            ply.div_ceil(2),
            if ply % 2 == 0 { "…" } else { "." },
            notation
        );
        moves = moves.child(
            ui::button(&format!("critical-{ply}"), &label, k)
                .on_press(move |s: &mut State| s.go_to(ply)),
        );
    }
    items = items.child(moves);
    ui::section("Momentos para aprender", items, k)
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
                Button::new(notation)
                    .variant(ButtonVariant::Ghost)
                    .control_size(ControlSize::ExtraLarge)
                    .build(k)
                    .key(format!("move-{ply}"))
                    .grow(1.0)
                    .min_w(0.0)
                    .fill(if s.ply == ply && s.active_variation.is_none() {
                        k.c.layer_01
                    } else {
                        Color::TRANSPARENT
                    })
                    .on_press(move |s: &mut State| s.go_to(ply)),
            );
            if s.preferences.feedback
                && let Some(class) = s.classification(ply)
            {
                cell = cell.child(review_feedback::badge(class));
            }
            line = line.child(cell);
        }
        list = list.child(line);
    }
    list
}
fn moves_panel(s: &State, k: &Kit) -> El {
    let mut content = col().gap(16.0);
    if s.real_result.is_some() {
        content = content.child(learning(s, k));
    }
    content = content.child(move_list(s, k));
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
    if let Some(error) = &s.bridge_error {
        return col()
            .key("analysis-error")
            .gap(8.0)
            .role("alert")
            .child(ui::label(error, 13.0, k.c.danger.fg).wrap().w_full())
            .child(
                ui::button("retry-analysis", "Tentar novamente", k).on_press(State::retry_review),
            );
    }
    if let Some((done, total)) = s.real_progress {
        return col()
            .gap(8.0)
            .child(ui::label(
                format!("Analisando {done}/{total}"),
                13.0,
                k.c.text_base,
            ))
            .child(
                Progress::new(Some(done as f32 / total.max(1) as f32))
                    .build(k)
                    .key("analysis-progress"),
            )
            .child(
                ui::button("cancel-analysis", "Cancelar análise", k).on_press(State::cancel_review),
            );
    }
    if s.real_result.is_some() {
        return if s.review_tab == ReviewTab::Summary {
            ui::label("Análise concluída", 13.0, k.c.text_base)
        } else {
            review_feedback::current(s, k).w_full()
        };
    }
    col()
        .gap(8.0)
        .child(
            ui::label(
                "Nenhuma análise concluída nesta sessão.",
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
        .child(ui::button("resume-analysis", "Retomar análise", k).on_press(State::retry_review))
}
fn inspector(s: &State, k: &Kit, stacked: bool) -> El {
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
        .role("region")
        .label("Painéis da revisão")
        .focus_return("focus-board")
        .min_h(0.0)
        .padding(16.0)
        .gap(12.0)
        .fill(k.c.bg_deep)
        .children((!stacked).then(|| {
            row()
                .center_y()
                .gap(8.0)
                .min_h(36.0)
                .child(ui::label("Análise local", 13.0, k.c.text_muted).grow(1.0))
                .child(engine_toggle(s, k))
                .child(panel_toggle(s, k))
        }))
        .children((!stacked).then_some(tabs))
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
                if s.real_result.is_some() {
                    "Stockfish · análise local"
                } else {
                    "PGN validado"
                },
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
}
fn engine_toggle(s: &State, k: &Kit) -> El {
    Switch::new(s.preferences.live_engine)
        .label("Linhas")
        .on_toggle(|s: &mut State| s.preferences.live_engine = !s.preferences.live_engine)
        .build(k)
        .key("review-engine-toggle")
}
fn toolbar(s: &State, k: &Kit, compact: bool) -> El {
    let selected = if !s.compact_panel {
        0
    } else {
        match s.review_tab {
            ReviewTab::Moves => 1,
            ReviewTab::Analysis => 2,
            ReviewTab::Summary => 3,
        }
    };
    let tabs = SegmentedControl::new(
        &["Tabuleiro", "Lances", "Análise", "Resumo"],
        Some(selected),
    )
    .label("Área da revisão")
    .width(None)
    .on_pick(|s: &mut State, i, _| {
        s.compact_panel = i != 0;
        if i != 0 {
            s.review_tab = match i {
                1 => ReviewTab::Moves,
                2 => ReviewTab::Analysis,
                _ => ReviewTab::Summary,
            };
        }
        s.playing = false;
    })
    .build(k)
    .key("compact-review-tabs")
    .grow(1.0)
    .min_w(0.0);
    row()
        .key("review-toolbar")
        .gap(8.0)
        .center_y()
        .px(12.0)
        .h(36.0)
        .children(compact.then_some(tabs))
        .child(engine_toggle(s, k))
}
pub fn view(s: &State, k: &Kit, layout: Layout, layers: &mut Vec<El>) -> El {
    review_analysis::dialog(s, k, layers);
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
        .children(
            (layout.stacked_review() || !s.review_sidebar.is_open()).then(|| panel_toggle(s, k)),
        )
        .child(
            ui::button("review-settings", "⋯", k)
                .label("Configurar análise")
                .on_press(|s: &mut State| s.analysis_settings_open = true),
        );
    let size = if s.review_sidebar.is_open() {
        layout.board_size()
    } else {
        layout.focus_board_size()
    };
    let board = workspace(s, k, size, layout.compact_height());
    let body = if !s.review_sidebar.is_open() {
        col()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .key("review-scroll")
            .child(board)
    } else if layout.stacked_review() {
        if s.compact_panel {
            col()
                .key("compact-panel")
                .grow(1.0)
                .min_h(0.0)
                .child(inspector(s, k, true).grow(1.0).min_h(0.0))
                .child(row().h(40.0).center().child(playback(s, k)))
        } else {
            col()
                .key("review-scroll")
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(board)
        }
    } else {
        col()
            .key("board-scroll")
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .child(board)
    };
    col()
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .child(header)
        .children(
            (layout.stacked_review() || !s.review_sidebar.is_open())
                .then(|| toolbar(s, k, layout.stacked_review() && s.review_sidebar.is_open())),
        )
        .child(ui::rule(k))
        .child(body)
}
