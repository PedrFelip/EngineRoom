//! Classification badges and feedback from the analyzed game.
use super::ui;
use crate::{review_data::Classification, state::State};
use rune::{Color, El, col, icon, row};
use rune_kit::{Kit, glyphs};
pub fn badge(class: Classification) -> El {
    let glyph = match class {
        Classification::Best => icon(&glyphs::STAR_FILLED, 14.0).color(Color::BLACK),
        Classification::Good => icon(&glyphs::CHECK_SMALL, 14.0).color(Color::BLACK),
        _ => ui::mono(class.glyph(), 12.0, Color::BLACK),
    };
    row()
        .role("img")
        .label(class.label())
        .size(22.0, 22.0)
        .radius(5.0)
        .center()
        .fill(class.color())
        .child(glyph.a11y_hidden())
}
pub fn current(s: &State, k: &Kit) -> El {
    let ply = s.displayed_ply();
    if !s.preferences.feedback
        || ply == 0
        || s.active_variation.is_some()
        || s.classification(ply).is_none()
    {
        return ui::label(
            "Mova uma peça para explorar uma variação legal.",
            12.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full();
    }
    let Some(class) = s.classification(ply) else {
        return ui::label("Feedback indisponível", 12.0, k.c.text_muted);
    };
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
                        "Δ {:.2}{}",
                        {
                            s.real_result
                                .as_ref()
                                .and_then(|r| r.moves.iter().find(|m| m.played.ply == ply))
                                .map_or(0.0, |m| m.cp_loss / 100.0)
                        },
                        { "" }
                    ),
                    12.0,
                    k.c.text_muted,
                )),
        )
}
