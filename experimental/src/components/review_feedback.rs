//! Classification badges and the current move's fictional review feedback.
use super::ui;
use crate::{
    review_data::{self, Classification},
    state::State,
};
use rune::{Color, El, col, row};
use rune_kit::Kit;
pub fn badge(class: Classification) -> El {
    row()
        .role("img")
        .label(class.label())
        .size(22.0, 22.0)
        .radius(5.0)
        .center()
        .fill(class.color())
        .child(ui::mono(class.glyph(), 12.0, Color::BLACK).a11y_hidden())
}
pub fn current(s: &State, k: &Kit) -> El {
    let ply = s.displayed_ply();
    if !s.is_mock() || !s.preferences.feedback || ply == 0 {
        return ui::label(
            if s.is_mock() {
                "Selecione um lance para ver seu feedback de demonstração."
            } else {
                "Mova uma peça para explorar uma variação legal."
            },
            12.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full();
    }
    let class = review_data::classification(ply);
    let move_name = match s.active_variation {
        Some(i) => s.variations[i]
            .cursor
            .checked_sub(1)
            .map(|j| s.variations[i].san[j].as_str()),
        None => s
            .game
            .moves
            .get(s.ply.saturating_sub(1))
            .map(String::as_str),
    }
    .unwrap_or("—");
    row()
        .key("move-feedback")
        .w_full()
        .gap(10.0)
        .center_y()
        .px(12.0)
        .py(8.0)
        .radius(8.0)
        .fill(class.color().alpha(0.1))
        .child(badge(class))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(3.0)
                .child(ui::label(
                    format!("{move_name} · {}", class.label()),
                    14.0,
                    k.c.text_base,
                ))
                .child(ui::label(
                    format!(
                        "Δ {:.2} · feedback fictício",
                        (review_data::cp(ply) - review_data::cp(ply.saturating_sub(1))).abs()
                            as f32
                            / 100.0
                    ),
                    12.0,
                    k.c.text_muted,
                )),
        )
}
