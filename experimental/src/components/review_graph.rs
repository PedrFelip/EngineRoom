//! Interactive fictional evaluation timeline and orientation-aware evaluation meter.
use super::ui;
use crate::{review_data, state::State};
use rune::{Color, El, LineCap, Phase, Shape, col, el, row};
use rune_kit::Kit;
pub fn meter(s: &State, k: &Kit, size: f32) -> El {
    let value = if s.is_mock() {
        review_data::win_pct(s.displayed_ply())
    } else {
        50.0
    };
    let white_height = size * value / 100.0;
    let top = if s.flipped { 0.0 } else { size - white_height };
    let label = if s.is_mock() {
        review_data::eval(review_data::cp(s.displayed_ply()))
    } else {
        "—".into()
    };
    el().key("evaluation-meter")
        .role("meter")
        .label("Avaliação pelas brancas · demonstração")
        .range_value(f64::from(value), 0.0, 100.0)
        .a11y_value(format!("{label}, {value:.1}%"))
        .size(26.0, size)
        .radius(5.0)
        .fill(Color::BLACK)
        .child(
            el().size(26.0, white_height)
                .absolute(0.0, top)
                .fill(Color::WHITE)
                .a11y_hidden(),
        )
        .child(
            ui::mono(label, 12.0, k.c.text_base)
                .absolute(0.0, size / 2.0 - 12.0)
                .w(26.0)
                .center()
                .fill(k.c.bg_base)
                .py(4.0),
        )
}
pub fn chart(s: &State, k: &Kit) -> El {
    const W: f32 = 320.0;
    const H: f32 = 110.0;
    let count = s.game.moves.len();
    let points: Vec<[f32; 2]> = (0..=count)
        .map(|i| {
            [
                i as f32 / count.max(1) as f32 * W,
                H * (1.0 - review_data::win_pct(i) / 100.0),
            ]
        })
        .collect();
    let mut area = points.clone();
    area.push([W, H]);
    area.push([0.0, H]);
    let x = s.ply as f32 / count.max(1) as f32 * W;
    let shape = Shape::new()
        .fill(area, k.c.text_base.alpha(0.07))
        .line(
            [0.0, H / 2.0],
            [W, H / 2.0],
            1.0,
            LineCap::Butt,
            k.c.border_strong,
        )
        .stroke(points, 2.0, k.c.text_base)
        .line([x, 0.0], [x, H], 1.5, LineCap::Butt, k.c.bg_accent)
        .view_box(W, H);
    col()
        .key("evaluation-chart-card")
        .gap(12.0)
        .w_full()
        .child(
            row()
                .justify_between()
                .center_y()
                .child(ui::label("Avaliação da partida", 14.0, k.c.text_base))
                .child(ui::mono(
                    review_data::eval(review_data::cp(s.ply)),
                    13.0,
                    k.c.text_base,
                )),
        )
        .child(
            el().key("evaluation-graph")
                .role("slider")
                .label("Gráfico de avaliação fictícia · selecione um lance")
                .range_value(s.ply as f64, 0.0, count as f64)
                .range_step(1.0)
                .focusable(true)
                .w_full()
                .h(H)
                .shape(shape)
                .on_step(|s: &mut State, steps, _| {
                    s.go_to(
                        (s.ply as i64 + steps as i64).clamp(0, s.game.moves.len() as i64) as usize,
                    )
                })
                .on_drag(|s: &mut State, d, _| {
                    if matches!(d.phase, Phase::Down | Phase::Move) {
                        s.go_to(
                            (d.x / d.w.max(1.0) * s.game.moves.len() as f32)
                                .round()
                                .clamp(0.0, s.game.moves.len() as f32)
                                as usize,
                        );
                    }
                }),
        )
        .child(
            row()
                .justify_between()
                .child(ui::label("Brancas / Pretas", 12.0, k.c.text_muted))
                .child(ui::mono(format!("Lance {}", s.ply), 12.0, k.c.text_muted)),
        )
        .child(
            ui::label(
                "Valores fictícios · clique ou use as setas no gráfico",
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
}
