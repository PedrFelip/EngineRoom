//! Persisted reviews from the experimental database.
use super::ui;
use crate::{layout::Layout, state::State};
use rune::{El, col, row};
use rune_kit::{ButtonVariant, Kit};

pub fn view(s: &State, k: &Kit, layout: Layout) -> El {
    let mut content = col().gap(24.0).child(ui::heading("Volte ao seu jogo", k));
    if s.history_loading {
        content = content.child(ui::label("Carregando histórico…", 12.0, k.c.text_muted));
    }
    if let Some(error) = &s.history_error {
        content = content.child(
            ui::label(error, 12.0, k.c.danger.fg)
                .role("alert")
                .wrap()
                .w_full(),
        );
    }
    if s.bridge.is_some() && s.history.is_empty() && !s.history_loading {
        content = content.child(ui::label("Nenhuma revisão salva.", 12.0, k.c.text_muted));
    }
    if s.history_total > 0 {
        content = content.child(
            ui::label(
                format!(
                    "Mostrando {} de {} revisões",
                    s.history.len(),
                    s.history_total
                ),
                12.0,
                k.c.text_muted,
            )
            .key("history-count"),
        );
    }
    for game in &s.history {
        let id = game.id;
        let entry = row()
            .flex_wrap()
            .gap(12.0)
            .center_y()
            .child(
                col()
                    .grow(1.0)
                    .min_w(220.0)
                    .gap(8.0)
                    .child(
                        ui::label(
                            format!("{} × {}", game.white, game.black),
                            18.0,
                            k.c.text_base,
                        )
                        .wrap()
                        .w_full(),
                    )
                    .child(
                        ui::label(
                            format!(
                                "{} · {} meios-lances · {}",
                                game.result, game.plies, game.created_at
                            ),
                            12.0,
                            k.c.text_muted,
                        )
                        .wrap()
                        .w_full(),
                    ),
            )
            .child(
                ui::button_trailing(
                    &format!("open-{id}"),
                    "Abrir revisão",
                    ButtonVariant::Neutral,
                    k,
                )
                .on_press(move |s: &mut State| s.history_open(id, false)),
            )
            .child(
                ui::button(&format!("reanalyze-{id}"), "Reanalisar", k)
                    .on_press(move |s: &mut State| s.history_open(id, true)),
            )
            .child(
                ui::button(&format!("delete-{id}"), "Excluir", k)
                    .on_press(move |s: &mut State| s.history_delete(id)),
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
    ui::page(content, layout)
}
