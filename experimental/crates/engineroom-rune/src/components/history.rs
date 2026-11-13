//! Persisted reviews from the experimental database.
#[path = "history_search.rs"]
mod history_search;

use super::ui;
use crate::{layout::Layout, state::State};
use rune::{El, col, el, row};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, TextInput};

pub fn view(s: &State, k: &Kit, layout: Layout) -> El {
    let mut content = col().gap(16.0).child(ui::heading("Histórico", k)).child(
        ui::label(
            "Revisões salvas, da mais recente para a mais antiga. A busca filtra apenas as partidas carregadas.",
            14.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full(),
    );
    content = content.child(
        TextInput::new(&s.history_filter)
            .label("Buscar jogador nas partidas carregadas")
            .placeholder("Nome de jogador")
            .height(40.0)
            .width(None)
            .on_edit(|s: &mut State| &mut s.history_filter)
            .build(k)
            .key("history-search"),
    );
    let search = history_search::HistorySearch::new(&s.history_filter.text);
    let filtered: Vec<_> = s
        .history
        .iter()
        .filter(|game| search.matches(&game.white, &game.black))
        .collect();
    if let Some(game) = s
        .history_confirm_delete
        .and_then(|id| s.history.iter().find(|game| game.id == id))
    {
        let id = game.id;
        let mut confirmation = col()
            .key(format!("history-confirm-{id}"))
            .focus_return(format!("delete-{id}"))
            .role("alertdialog")
            .gap(10.0)
            .child(ui::label(
                "Excluir esta revisão do histórico?",
                16.0,
                k.c.text_base,
            ))
            .child(
                ui::label(
                    format!("{} × {}", game.white, game.black),
                    14.0,
                    k.c.text_base,
                )
                .wrap(),
            )
            .child(
                ui::label(
                    "A revisão salva será removida deste histórico.",
                    12.0,
                    k.c.text_muted,
                )
                .wrap(),
            );
        if let Some(error) = &s.history_delete_error {
            confirmation =
                confirmation.child(ui::label(error, 12.0, k.c.danger.fg).role("alert").wrap());
        }
        confirmation = confirmation.child(
            row()
                .flex_wrap()
                .gap(8.0)
                .child(
                    Button::new("Cancelar")
                        .control_size(ControlSize::Large)
                        .build(k)
                        .key("history-delete-cancel")
                        .autofocus()
                        .disabled(s.history_delete_pending)
                        .on_press(State::cancel_history_delete),
                )
                .child(
                    Button::new(if s.history_delete_pending {
                        "Excluindo…"
                    } else {
                        "Excluir revisão"
                    })
                    .control_size(ControlSize::Large)
                    .variant(ButtonVariant::Danger)
                    .disabled(s.history_delete_pending)
                    .build(k)
                    .key("history-delete-confirm")
                    .on_press(State::confirm_history_delete),
                ),
        );
        content = content.child(ui::group(confirmation, k).max_w(520.0));
    }
    if s.history_loading {
        content = content.child(ui::label("Carregando histórico…", 12.0, k.c.text_muted));
    }
    if let Some(error) = &s.history_error {
        content = content.child(
            ui::label(
                format!("{}: {error}", s.history_error_context()),
                12.0,
                k.c.danger.fg,
            )
            .role("alert")
            .wrap()
            .w_full(),
        );
    }
    if show_empty_history(
        s.bridge.is_some(),
        s.history.is_empty(),
        s.history_loading,
        s.history_error.is_some(),
    ) {
        content = content.child(ui::label("Nenhuma revisão salva.", 12.0, k.c.text_muted));
    }
    if show_empty_search(
        &s.history_filter.text,
        s.history.is_empty(),
        filtered.is_empty(),
        s.history_loading,
        s.history_error.is_some(),
    ) {
        let message = if s.history_cursor.is_some() {
            "Nenhuma partida carregada corresponde à busca. Carregue mais para continuar buscando."
        } else {
            "Nenhuma partida corresponde à busca."
        };
        content = content.child(ui::label(message, 12.0, k.c.text_muted).wrap().w_full());
    }
    if s.history_error.is_some() && !s.history_loading {
        content = content.child(
            ui::button("history-retry", "Tentar novamente", k).on_press(State::retry_history),
        );
    }
    if s.history_total > 0 {
        content = content.child(
            ui::label(
                format!(
                    "{} correspondentes entre {} de {} revisões carregadas",
                    filtered.len(),
                    s.history.len(),
                    s.history_total
                ),
                12.0,
                k.c.text_muted,
            )
            .key("history-count"),
        );
    }
    for game in filtered {
        let id = game.id;
        let analysis = match game.analysis_kind.as_str() {
            "auto-fast" => "Automático · Rápido".to_string(),
            "auto-deep" => "Automático · Profundo".to_string(),
            _ if game.mode == engineroom_backend::db::mode::Mode::Time => {
                format!("Tempo · {:.1} s/lance", game.depth as f64 / 1000.0)
            }
            _ => format!("Profundidade · {}", game.depth),
        };
        let entry = col()
            .w_full()
            .gap(14.0)
            .child(
                row()
                    .w_full()
                    .flex_wrap()
                    .center_y()
                    .gap(10.0)
                    .child(
                        ui::label(
                            format!("{} × {}", game.white, game.black),
                            18.0,
                            k.c.text_base,
                        )
                        .wrap()
                        .grow(1.0)
                        .min_w(180.0),
                    )
                    .child(
                        ui::label(&game.result, 14.0, k.c.text_base)
                            .padding(8.0)
                            .radius(6.0)
                            .fill(k.c.layer_02),
                    ),
            )
            .child(
                ui::label(
                    format!("{} meios-lances · {} (UTC)", game.plies, game.created_at),
                    12.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            )
            .child(ui::rule(k))
            .child(
                row()
                    .w_full()
                    .flex_wrap()
                    .gap(20.0)
                    .child(
                        col()
                            .gap(4.0)
                            .child(ui::label("PRECISÃO", 12.0, k.c.text_muted))
                            .child(
                                row()
                                    .flex_wrap()
                                    .center_y()
                                    .gap(14.0)
                                    .child(accuracy("Brancas", game.accuracy_white, true, k))
                                    .child(accuracy("Pretas", game.accuracy_black, false, k)),
                            ),
                    )
                    .child(
                        col()
                            .gap(4.0)
                            .child(ui::label("ANÁLISE", 12.0, k.c.text_muted))
                            .child(
                                ui::label(
                                    format!("{} · {} linhas", analysis, game.multipv),
                                    14.0,
                                    k.c.text_base,
                                )
                                .wrap(),
                            ),
                    ),
            )
            .child(
                row()
                    .w_full()
                    .flex_wrap()
                    .gap(8.0)
                    .child(
                        Button::new("Abrir revisão")
                            .control_size(ControlSize::Large)
                            .variant(ButtonVariant::Neutral)
                            .build(k)
                            .key(format!("open-{id}"))
                            .on_press(move |s: &mut State| s.history_open(id, false)),
                    )
                    .child(
                        Button::new("Reanalisar")
                            .control_size(ControlSize::Large)
                            .build(k)
                            .key(format!("reanalyze-{id}"))
                            .on_press(move |s: &mut State| s.history_open(id, true)),
                    )
                    .child(
                        Button::new("Excluir")
                            .control_size(ControlSize::Large)
                            .build(k)
                            .key(format!("delete-{id}"))
                            .disabled(s.history_delete_pending)
                            .on_press_tx(move |s: &mut State, tx| {
                                s.request_history_delete(id);
                                tx.scroll_into_view("history-delete-confirm");
                            }),
                    ),
            );
        content = content.child(ui::group(entry, k));
    }
    if s.history_cursor.is_some() {
        content = content.child(
            ui::button("history-more", "Carregar mais", k)
                .disabled(s.history_loading)
                .on_press(State::next_history),
        );
    }
    ui::page(content, layout.padding(), 800.0)
}

fn accuracy(label: &str, value: f64, white: bool, k: &Kit) -> El {
    row()
        .center_y()
        .gap(6.0)
        .child(
            el().size(12.0, 12.0)
                .radius(6.0)
                .fill(if white { k.c.white } else { k.c.black })
                .border(1.0, k.c.border_strong)
                .a11y_hidden(),
        )
        .child(ui::label(
            format!("{label} {value:.1}%"),
            14.0,
            k.c.text_base,
        ))
}

fn show_empty_history(connected: bool, empty: bool, loading: bool, error: bool) -> bool {
    connected && empty && !loading && !error
}

fn show_empty_search(
    query: &str,
    history_empty: bool,
    filtered_empty: bool,
    loading: bool,
    error: bool,
) -> bool {
    !query.trim().is_empty() && !history_empty && filtered_empty && !loading && !error
}

#[cfg(test)]
mod tests {
    use super::{show_empty_history, show_empty_search};

    #[test]
    fn confirmation_actions_remain_usable_at_supported_sizes() {
        use crate::{spec, state::Page};
        use rune::{Env, Key, KeyEvent};

        for size in [(1920.0, 1080.0), (900.0, 700.0), (720.0, 560.0)] {
            let mut rt = rune::App::new(spec()).headless_runtime(
                size,
                Env {
                    system_dark: true,
                    scale: 1.0,
                },
            );
            rt.state.page = Page::History;
            rt.state
                .history
                .push(engineroom_backend::db::games::GameSummary {
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
            rt.state.history_total = 1;
            rt.state.request_history_delete(1);
            rt.invalidate();
            rt.frame(0.0);
            for name in ["history-delete-cancel", "history-delete-confirm"] {
                let (x, y) = rt.find(name).expect("confirmation action exists");
                assert!(
                    x > 0.0 && x < size.0 && y > 0.0 && y < size.1,
                    "{name} is visible at {size:?}: {x}, {y}"
                );
            }
            rt.set_tab_visits_controls(true);
            for _ in 0..100 {
                if rt
                    .elements()
                    .iter()
                    .any(|e| e.focused && e.key.as_deref() == Some("history-delete-confirm"))
                {
                    break;
                }
                rt.key(KeyEvent {
                    key: Key::Tab,
                    mods: Default::default(),
                });
                rt.frame(1.0 / 60.0);
            }
            assert!(
                rt.elements()
                    .iter()
                    .any(|e| e.focused && e.key.as_deref() == Some("history-delete-confirm"))
            );
            rt.key(KeyEvent {
                key: Key::Text(" ".into()),
                mods: Default::default(),
            });
            rt.frame(0.0);
            assert!(
                rt.state.history_delete_error.is_some(),
                "unavailable bridge reports an error"
            );
            rt.key(KeyEvent {
                key: Key::Text(" ".into()),
                mods: Default::default(),
            });
            rt.frame(0.0);
            assert!(
                rt.state.history_delete_error.is_some(),
                "retry remains available"
            );
            rt.state.history_delete_pending = true;
            rt.invalidate();
            rt.frame(0.0);
            rt.key(KeyEvent {
                key: Key::Escape,
                mods: Default::default(),
            });
            rt.frame(0.0);
            assert_eq!(
                rt.state.history_confirm_delete,
                Some(1),
                "pending deletion cannot be cancelled"
            );
            rt.state.history_delete_pending = false;
            rt.invalidate();
            rt.frame(0.0);
            rt.key(KeyEvent {
                key: Key::Escape,
                mods: Default::default(),
            });
            rt.frame(0.0);
            assert_eq!(
                rt.state.history_confirm_delete, None,
                "escape cancels after failure"
            );
        }
    }

    #[test]
    fn empty_messages_do_not_mask_loading_or_errors() {
        assert!(show_empty_history(true, true, false, false));
        assert!(!show_empty_history(true, true, false, true));
        assert!(!show_empty_history(true, true, true, false));
        assert!(!show_empty_search("ana", true, true, false, false));
        assert!(!show_empty_search("ana", false, true, false, true));
        assert!(show_empty_search("ana", false, true, false, false));
    }
}
