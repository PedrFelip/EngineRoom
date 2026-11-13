//! Full review presentation with deterministic fixture scenes and legal board exploration.
use super::{board, review_analysis, review_feedback, review_graph, review_summary, ui};
use crate::{
    layout::Layout,
    state::{LearningSide, ReviewTab, State},
};
use rune::{Color, Cursor, Drag, El, Icon, Phase, ScrollSpec, col, el, row};
use rune_kit::{
    Button, ButtonVariant, ControlSize, Dialog, IconButton, IconButtonVariant, Kit, Progress,
    SegmentedControl, Switch,
};
use shakmaty::Position;

// Primer Octicons, MIT; attribution is bundled in assets/OCTICONS-LICENSE.txt.
static SIDEBAR_EXPAND: Icon = Icon {
    name: "sidebar-expand",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../../../assets/sidebar-expand-16.svg"),
        "</g>"
    ),
};
static SIDEBAR_COLLAPSE: Icon = Icon {
    name: "sidebar-collapse",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../../../assets/sidebar-collapse-16.svg"),
        "</g>"
    ),
};

fn panel_toggle(s: &State, k: &Kit) -> El {
    ui::icon_feedback(
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
        .build(k),
        k,
    )
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
octicon!(
    FIRST,
    "../../../../assets/move-to-start-16.svg",
    "move-to-start"
);
octicon!(
    PREVIOUS,
    "../../../../assets/chevron-left-16.svg",
    "chevron-left"
);
octicon!(
    NEXT,
    "../../../../assets/chevron-right-16.svg",
    "chevron-right"
);
octicon!(LAST, "../../../../assets/move-to-end-16.svg", "move-to-end");
octicon!(
    FLIP,
    "../../../../assets/arrow-switch-16.svg",
    "arrow-switch"
);

fn playback(s: &State, k: &Kit) -> El {
    let (position, total) = s.active_variation.map_or((s.ply, s.game.moves.len()), |i| {
        let variation = &s.variations[i];
        let start = variation.branch_from.map_or(0, |(_, cursor)| cursor);
        (
            variation.cursor.saturating_sub(start),
            variation.moves.len().saturating_sub(start),
        )
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
        ui::icon_feedback(
            IconButton::new(icon)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Large)
                .label(label)
                .help(label)
                .shortcut(shortcut)
                .disabled(disabled)
                .on_press_tx(move |s: &mut State, tx| {
                    action(s);
                    crate::reveal_move(s, tx);
                })
                .build(k),
            k,
        )
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
                    position == 0 && s.active_variation.is_none(),
                    State::previous,
                ))
                .child(
                    ui::mono(counter, 12.0, k.c.text_muted)
                        .key("move-counter")
                        .role("status")
                        .label(s.active_variation.map_or_else(
                            || format!("Meio-lance {position} de {total}"),
                            |i| format!("Variação {}, meio-lance {position} de {total}", i + 1),
                        ))
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
            ui::icon_feedback(
                IconButton::new(&FLIP)
                    .variant(IconButtonVariant::GhostMuted)
                    .control_size(ControlSize::Large)
                    .label("Girar tabuleiro")
                    .help("Girar tabuleiro")
                    .shortcut("F")
                    .on_press(|s: &mut State| s.flipped = !s.flipped)
                    .build(k),
                k,
            )
            .key("flip")
            .checked(s.flipped)
            .size(32.0, 32.0),
        )
        .children(s.active_variation.map(|_| {
            ui::icon_feedback(
                IconButton::new(&super::icons::RETURN)
                    .variant(IconButtonVariant::GhostMuted)
                    .control_size(ControlSize::Large)
                    .label("Voltar à partida")
                    .help("Voltar à partida")
                    .on_press_tx(|s: &mut State, tx| {
                        s.selected_square = None;
                        s.go_to(s.ply);
                        crate::reveal_move(s, tx);
                    })
                    .build(k),
                k,
            )
            .key("return-mainline")
            .size(32.0, 32.0)
        }))
        .children(s.active_variation.map(|_| {
            ui::icon_feedback(
                IconButton::new(if s.playing {
                    &super::icons::PAUSE
                } else {
                    &super::icons::PLAY
                })
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Large)
                .label(if s.playing { "Pausar" } else { "Reproduzir" })
                .on_press_tx(|s: &mut State, tx| {
                    s.toggle_playback();
                    tx.wake_after(std::time::Duration::from_millis(50));
                })
                .build(k),
                k,
            )
            .key("pause-variation")
            .size(32.0, 32.0)
        }))
}
fn workspace(s: &State, k: &Kit, size: f32, compact: bool) -> El {
    let (top, bottom) = if s.flipped {
        ((&s.game.white, "Brancas"), (&s.game.black, "Pretas"))
    } else {
        ((&s.game.black, "Pretas"), (&s.game.white, "Brancas"))
    };
    col()
        .key("board-workspace")
        .w_full()
        .items_center()
        .gap(4.0)
        .py(if compact { 2.0 } else { 8.0 })
        .child(board::player(
            top.0,
            top.1,
            s.avatar(if s.flipped { 0 } else { 1 }),
            &s.game.ratings[if s.flipped { 0 } else { 1 }],
            k,
            size + 24.0,
            compact,
        ))
        .child(
            col()
                .key("position-evaluation")
                .items_center()
                .gap(0.0)
                .child(review_graph::readout(s, k, size))
                .child(
                    row()
                        .gap(10.0)
                        .items_start()
                        .child(review_graph::meter(s, k, size))
                        .child(board::view(s, k, size)),
                ),
        )
        .child(board::player(
            bottom.0,
            bottom.1,
            s.avatar(if s.flipped { 1 } else { 0 }),
            &s.game.ratings[if s.flipped { 1 } else { 0 }],
            k,
            size + 24.0,
            compact,
        ))
        .child(playback(s, k))
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
            s.game.positions[ply - 1].fullmoves(),
            if s.game.positions[ply - 1].turn().is_black() {
                "…"
            } else {
                "."
            },
            notation
        );
        moves = moves.child(
            ui::button(&format!("critical-{ply}"), &label, k).on_press_tx(
                move |s: &mut State, tx| {
                    s.go_to(ply);
                    crate::reveal_move(s, tx);
                },
            ),
        );
    }
    items = items.child(moves);
    ui::section("Momentos para aprender", items, k)
}
fn branches_at(s: &State, k: &Kit, root_ply: usize, indices: &[usize]) -> Option<El> {
    if indices.is_empty() {
        return None;
    }
    let mut branches = col()
        .key(format!("branches-{root_ply}"))
        .gap(8.0)
        .padding(10.0)
        .radius(k.radius.md)
        .border(1.0, k.c.border_base)
        .fill(k.c.layer_01);
    for &i in indices {
        let v = &s.variations[i];
        let mut moves = row().gap(8.0).flex_wrap();
        let branch_start = v.branch_from.map_or(0, |(_, cursor)| cursor);
        for (offset, san) in v.san.iter().enumerate().skip(branch_start) {
            let position = &v.positions[offset];
            let caption = format!(
                "{}{} {}",
                position.fullmoves(),
                if position.turn().is_black() {
                    "…"
                } else {
                    "."
                },
                san
            );
            let selected = s.visible_variation_cursor() == Some((i, offset + 1));
            let mut move_cell = row()
                .key(format!("variation-{i}-cell-{}", offset + 1))
                .gap(4.0)
                .center_y()
                .radius(k.radius.md)
                .motion(rune::Spring::new(0.18, 0.0))
                .fill(if selected {
                    k.c.bg_accent.alpha(0.25)
                } else {
                    Color::TRANSPARENT
                });
            move_cell = move_cell.child(
                Button::new(&caption)
                    .variant(ButtonVariant::Ghost)
                    .control_size(ControlSize::Large)
                    .build(k)
                    .key(format!("variation-{i}-move-{}", offset + 1))
                    .label(format!(
                        "Variação {}, {caption}{}",
                        i + 1,
                        if selected { ", lance atual" } else { "" }
                    ))
                    .fill(Color::TRANSPARENT)
                    .on_press_tx(move |s: &mut State, tx| {
                        s.go_to_variation(i, offset + 1);
                        crate::reveal_move(s, tx);
                    }),
            );
            if s.preferences.feedback
                && let Some(Some(class)) = v.classifications.get(offset)
            {
                move_cell = move_cell.child(review_feedback::badge(
                    crate::state::display_classification(*class),
                ));
            }
            moves = moves.child(move_cell);
        }
        let origin = if let Some((parent, cursor)) = v.branch_from {
            if let Some(previous) = cursor.checked_sub(1) {
                let position = &v.positions[previous];
                format!(
                    "ramo de V{} após {}{} {}",
                    parent + 1,
                    position.fullmoves(),
                    if position.turn().is_black() {
                        "…"
                    } else {
                        "."
                    },
                    v.san[previous]
                )
            } else {
                format!("ramo de V{} na origem", parent + 1)
            }
        } else if v.root_ply == 0 {
            "posição inicial".to_string()
        } else {
            let before = &s.game.positions[v.root_ply - 1];
            format!(
                "após {}{} {}",
                before.fullmoves(),
                if before.turn().is_black() { "…" } else { "." },
                s.game.moves[v.root_ply - 1]
            )
        };
        branches = branches.child(
            col()
                .gap(4.0)
                .child(ui::label(
                    format!("Variação {} · {origin}", i + 1),
                    12.0,
                    k.c.text_muted,
                ))
                .child(moves),
        );
    }
    Some(branches)
}
fn move_list(s: &State, k: &Kit) -> El {
    // Transient render data: group once rather than scan every branch for every ply.
    let mut branches = vec![Vec::new(); s.game.moves.len() + 1];
    for (index, variation) in s.variations.iter().enumerate() {
        if let Some(group) = branches.get_mut(variation.root_ply) {
            group.push(index);
        }
    }
    let mut list = col().key("move-list").w_full().gap(4.0);
    if let Some(branches) = branches_at(s, k, 0, &branches[0]) {
        list = list.child(branches);
    }
    let mut index = 0;
    while index < s.game.moves.len() {
        let position = &s.game.positions[index];
        let mut line = row().gap(6.0).center_y().child(
            ui::mono(
                format!(
                    "{}{}",
                    position.fullmoves(),
                    if position.turn().is_black() {
                        "…"
                    } else {
                        "."
                    }
                ),
                12.0,
                k.c.text_muted,
            )
            .w(32.0),
        );
        if position.turn().is_black() {
            line = line.child(el().grow(1.0));
        }
        for _ in 0..if position.turn().is_black() { 1 } else { 2 } {
            if index >= s.game.moves.len() {
                break;
            }
            let ply = index + 1;
            let notation = &s.game.moves[index];
            let selected = s.ply == ply
                && s.active_variation
                    .is_none_or(|i| s.variations[i].cursor == 0);
            let mut cell = k
                .interactive(row())
                .key(format!("move-{ply}"))
                .role("button")
                .focusable(true)
                .label(format!(
                    "{}{} {}{}",
                    position.fullmoves(),
                    if s.game.positions[index].turn().is_black() {
                        "…"
                    } else {
                        "."
                    },
                    notation,
                    if selected { ", lance atual" } else { "" }
                ))
                .grow(1.0)
                .min_w(0.0)
                .h(36.0)
                .px(8.0)
                .gap(5.0)
                .center_y()
                .radius(k.radius.md)
                .fill(if selected {
                    k.c.bg_accent.alpha(0.25)
                } else {
                    Color::TRANSPARENT
                })
                .child(
                    rune::text(notation)
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .grow(1.0)
                        .center()
                        .a11y_hidden(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    s.go_to(ply);
                    crate::reveal_move(s, tx);
                });
            if s.preferences.feedback
                && let Some(class) = s.classification(ply)
            {
                cell = cell.child(review_feedback::badge(class));
            }
            line = line.child(cell);
            index += 1;
            if !branches[index].is_empty() {
                if s.game.positions[index - 1].turn().is_white() {
                    line = line.child(el().grow(1.0));
                }
                break;
            }
        }
        list = list.child(line);
        if let Some(branches) = branches_at(s, k, index, &branches[index]) {
            list = list.child(branches);
        }
    }
    list
}
fn moves_panel(s: &State, k: &Kit) -> El {
    let mut content = col().gap(16.0);
    if s.real_result.is_some() {
        content = content.child(learning(s, k));
    }
    content = content.child(move_list(s, k));
    content
}
fn status(s: &State, k: &Kit) -> El {
    if let Some(error) = &s.bridge_error {
        return col()
            .key("analysis-error")
            .gap(8.0)
            .role("alert")
            .child(
                ui::label("Não foi possível concluir a análise.", 13.0, k.c.text_base)
                    .wrap()
                    .w_full(),
            )
            .child(ui::label(error, 13.0, k.c.danger.fg).wrap().w_full())
            .child(
                ui::button("retry-analysis", "Tentar novamente", k).on_press(State::retry_review),
            );
    }
    if let Some((done, total)) = s.real_progress {
        return col()
            .gap(8.0)
            .child(ui::analysis_label(
                &format!("Analisando posições: {done} de {total}"),
                rune::TextStyle::new(13.0, 400.0),
                s,
                k,
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
        .on_pick(|s: &mut State, i, tx| {
            s.review_tab = match i {
                0 => ReviewTab::Moves,
                1 => ReviewTab::Analysis,
                _ => ReviewTab::Summary,
            };
            s.playing = false;
            if i == 0 {
                crate::reveal_move(s, tx);
            }
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
        .on_toggle(|s: &mut State| s.toggle_live_engine())
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
    .on_pick(|s: &mut State, i, tx| {
        s.compact_panel = i != 0;
        if i != 0 {
            s.review_tab = match i {
                1 => ReviewTab::Moves,
                2 => ReviewTab::Analysis,
                _ => ReviewTab::Summary,
            };
        }
        s.playing = false;
        if i == 1 {
            crate::reveal_move(s, tx);
        }
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
    let mut choices = col().gap(8.0);
    let mut choice_row = row().gap(8.0);
    for (index, (name, role, icon)) in [
        ("Dama", shakmaty::Role::Queen, &super::pieces::QUEEN_BODY),
        ("Torre", shakmaty::Role::Rook, &super::pieces::ROOK_BODY),
        ("Bispo", shakmaty::Role::Bishop, &super::pieces::BISHOP_BODY),
        (
            "Cavalo",
            shakmaty::Role::Knight,
            &super::pieces::KNIGHT_BODY,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        choice_row = choice_row.child(
            Button::new(name)
                .icon(icon)
                .control_size(ControlSize::ExtraLarge)
                .build(k)
                .key(format!("promote-{name}"))
                .grow(1.0)
                .on_press_tx(move |s: &mut State, tx| {
                    s.promote(role);
                    crate::reveal_move(s, tx);
                }),
        );
        if index % 2 == 1 {
            choices = choices.child(choice_row);
            choice_row = row().gap(8.0);
        }
    }
    Dialog::new("promotion", "Escolha a promoção")
        .open(s.promotion_pending.is_some())
        .width(440.0)
        .fit()
        .on_close(|s: &mut State| s.promotion_pending = None)
        .child(choices.padding(16.0))
        .build(k, layers);
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
