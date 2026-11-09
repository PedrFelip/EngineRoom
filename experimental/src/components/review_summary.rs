//! Accuracy, phase breakdown and classification counts from the analyzed game.
use super::{review_feedback, review_graph, ui};
use crate::{review_data::CLASSES, state::State};
use rune::{Color, El, col, el, row};
use rune_kit::Kit;
pub fn view(s: &State, k: &Kit) -> El {
    if let Some(result) = &s.real_result {
        let mut counts = col().gap(8.0);
        for class in CLASSES {
            let count = |side: &str| {
                result
                    .moves
                    .iter()
                    .filter(|m| {
                        m.played.color == side && s.classification(m.played.ply) == Some(class)
                    })
                    .count()
            };
            counts = counts.child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(review_feedback::badge(class))
                    .child(ui::label(class.label(), 13.0, k.c.text_base).grow(1.0))
                    .child(ui::mono(
                        format!("{} / {}", count("w"), count("b")),
                        13.0,
                        k.c.text_base,
                    )),
            );
        }
        return col()
            .key("review-summary")
            .gap(16.0)
            .child(review_graph::chart(s, k))
            .child(accuracy(result.accuracy.white, result.accuracy.black, k))
            .child(ui::section(
                "Qualidade dos lances · Brancas / Pretas",
                counts,
                k,
            ))
            .child(ui::label(
                format!("{} lances analisados", result.moves.len()),
                13.0,
                k.c.text_muted,
            ));
    }
    ui::label(
        "Leve a partida à análise para ver o resumo.",
        14.0,
        k.c.text_muted,
    )
    .wrap()
    .w_full()
}

fn accuracy(white: f64, black: f64, k: &Kit) -> El {
    let panel = |label: &str, value: f64, marker: Color| {
        col()
            .grow(1.0)
            .min_w(0.0)
            .gap(8.0)
            .padding(14.0)
            .radius(10.0)
            .fill(k.c.bg_base)
            .border(1.0, k.c.border_base)
            .child(
                row()
                    .center_y()
                    .gap(7.0)
                    .child(
                        el().size(9.0, 9.0)
                            .radius(5.0)
                            .fill(marker)
                            .border(1.0, k.c.border_strong)
                            .a11y_hidden(),
                    )
                    .child(ui::label(label, 12.0, k.c.text_muted)),
            )
            .child(ui::label(format!("{value:.1}%"), 24.0, k.c.text_base))
    };
    col()
        .gap(10.0)
        .w_full()
        .child(ui::label("Acurácia", 13.0, k.c.text_muted))
        .child(
            row()
                .w_full()
                .gap(8.0)
                .child(panel("Brancas", white, Color::WHITE))
                .child(panel("Pretas", black, Color::BLACK)),
        )
}
