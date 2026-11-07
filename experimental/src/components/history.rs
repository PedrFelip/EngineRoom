//! Session-local imported game, presented like the Tauri history entry point.
use super::ui;
use crate::{
    layout::Layout,
    state::{Page, State},
};
use rune::{El, col, row};
use rune_kit::Kit;
pub fn view(s: &State, k: &Kit, layout: Layout) -> El {
    let game = col()
        .grow(1.0)
        .min_w(220.0)
        .gap(8.0)
        .child(
            ui::label(
                format!("{} × {}", s.game.white, s.game.black),
                18.0,
                k.c.text_base,
            )
            .wrap()
            .w_full(),
        )
        .child(
            ui::label(
                format!("{} · {} meios-lances", s.game.event, s.game.moves.len()),
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        );
    let entry = row().flex_wrap().gap(20.0).center_y().child(game).child(
        ui::button("open-demo", "Abrir revisão →", k)
            .on_press(|s: &mut State| s.navigate(Page::Review)),
    );
    ui::page(col().gap(24.0).child(ui::heading("Volte ao seu jogo",k))
        .child(ui::label("Partida disponível nesta sessão",12.0,k.c.text_muted))
        .child(ui::group(entry,k))
        .child(ui::label("O histórico persistente será fornecido pelo backend do EngineRoom. Esta lista mostra somente a prévia aberta na sessão atual.",12.0,k.c.text_muted).wrap().w_full()),layout)
}
