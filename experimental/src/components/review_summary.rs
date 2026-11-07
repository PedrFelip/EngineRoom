//! Fictional accuracy, phase breakdown and classification counts, derived from one fixture.
use super::{review_feedback, review_graph, ui};
use crate::{
    review_data::{self, CLASSES},
    state::State,
};
use rune::{El, col, row};
use rune_kit::Kit;
fn score(value: Option<f32>) -> String {
    value.map_or_else(|| "—".into(), |v| format!("{v:.1}%"))
}
fn metric(title: &str, value: String, k: &Kit) -> El {
    col()
        .grow(1.0)
        .min_w(0.0)
        .gap(8.0)
        .child(ui::label(title, 12.0, k.c.text_muted))
        .child(ui::mono(value, 24.0, k.c.text_base))
}
pub fn view(s: &State, k: &Kit) -> El {
    if !s.is_mock() {
        return ui::label(
            "Ative o modo demonstração para visualizar gráficos e estatísticas fictícias.",
            14.0,
            k.c.text_muted,
        )
        .wrap()
        .w_full();
    }
    let len = s.game.moves.len();
    let accuracy = row()
        .gap(20.0)
        .child(metric(
            "Brancas",
            score(review_data::accuracy((1..=len).filter(|p| p % 2 == 1))),
            k,
        ))
        .child(metric(
            "Pretas",
            score(review_data::accuracy((1..=len).filter(|p| p % 2 == 0))),
            k,
        ));
    let mut phases = col().gap(12.0).child(
        row()
            .child(ui::label("Fase · amostra fictícia", 12.0, k.c.text_muted).grow(1.0))
            .child(ui::label("Br. / Pr.", 12.0, k.c.text_muted)),
    );
    for (label, start, end) in [
        ("Abertura", 1, len / 3),
        ("Meio-jogo", len / 3 + 1, len * 2 / 3),
        ("Final", len * 2 / 3 + 1, len),
    ] {
        phases = phases.child(
            row()
                .gap(8.0)
                .child(ui::label(label, 13.0, k.c.text_base).grow(1.0))
                .child(
                    ui::mono(
                        score(review_data::accuracy((start..=end).filter(|p| p % 2 == 1))),
                        12.0,
                        k.c.text_base,
                    )
                    .w(54.0),
                )
                .child(
                    ui::mono(
                        score(review_data::accuracy((start..=end).filter(|p| p % 2 == 0))),
                        12.0,
                        k.c.text_base,
                    )
                    .w(54.0),
                ),
        );
    }
    let mut counts = col().gap(8.0).child(
        row()
            .child(ui::label("Classificação", 12.0, k.c.text_muted).grow(1.0))
            .child(ui::label("Br. / Pr.", 12.0, k.c.text_muted)),
    );
    for class in CLASSES {
        let count = |side| {
            (1..=len)
                .filter(|p| p % 2 == side && review_data::classification(*p) == class)
                .count()
        };
        counts = counts.child(
            row()
                .gap(8.0)
                .center_y()
                .child(review_feedback::badge(class))
                .child(ui::label(class.label(), 13.0, k.c.text_base).grow(1.0))
                .child(ui::mono(count(1).to_string(), 13.0, k.c.text_base).w(54.0))
                .child(ui::mono(count(0).to_string(), 13.0, k.c.text_base).w(54.0)),
        );
    }
    col()
        .key("review-summary")
        .gap(20.0)
        .child(ui::group(review_graph::chart(s, k), k))
        .child(ui::section("Acurácia geral · mock", accuracy, k))
        .child(ui::section("Acurácia por fase · mock", phases, k))
        .child(ui::section("Qualidade dos lances", counts, k))
}
