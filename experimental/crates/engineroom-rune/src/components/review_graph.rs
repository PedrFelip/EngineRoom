//! Interactive evaluation timeline and orientation-aware evaluation meter.
use super::ui;
use crate::{
    review_data,
    state::{Appearance, State},
};
use rune::{Color, El, LineCap, Phase, Shape, col, el, oklch, row};
use rune_kit::Kit;
fn smooth(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut result = vec![points[0]];
    for pair in points.windows(2) {
        for step in 1..=8 {
            let t = step as f32 / 8.0;
            let eased = t * t * (3.0 - 2.0 * t);
            result.push([
                pair[0][0] + (pair[1][0] - pair[0][0]) * t,
                pair[0][1] + (pair[1][1] - pair[0][1]) * eased,
            ]);
        }
    }
    result
}

// Split at crossings so each filled polygon is simple and can be triangulated by Shape.
fn split_midline(points: &[[f32; 2]], mid: f32) -> Vec<(bool, Vec<[f32; 2]>)> {
    let Some(&first) = points.first() else {
        return Vec::new();
    };
    let mut above = first[1] <= mid;
    let mut current = vec![first];
    let mut parts = Vec::new();
    for pair in points.windows(2) {
        let next_above = pair[1][1] <= mid;
        if next_above != above {
            let t = (mid - pair[0][1]) / (pair[1][1] - pair[0][1]);
            let crossing = [pair[0][0] + t * (pair[1][0] - pair[0][0]), mid];
            current.push(crossing);
            parts.push((above, current));
            current = vec![crossing];
            above = next_above;
        }
        current.push(pair[1]);
    }
    parts.push((above, current));
    parts
}

fn dashed(mut shape: Shape, a: [f32; 2], b: [f32; 2], color: Color) -> Shape {
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    let mut offset = 0.0_f32;
    while offset < length {
        let point = |distance: f32| {
            [
                a[0] + (b[0] - a[0]) * distance / length,
                a[1] + (b[1] - a[1]) * distance / length,
            ]
        };
        shape = shape.line(
            point(offset),
            point((offset + 2.0).min(length)),
            0.75,
            LineCap::Butt,
            color,
        );
        offset += 6.0;
    }
    shape
}

fn circle_pts(cx: f32, cy: f32, radius: f32) -> Vec<[f32; 2]> {
    (0..32)
        .map(|i| {
            let angle = i as f32 * std::f32::consts::TAU / 32.0;
            [cx + radius * angle.cos(), cy + radius * angle.sin()]
        })
        .collect()
}

fn phases(mut shape: Shape, count: usize, width: f32, height: f32, color: Color) -> Shape {
    let scale = width / count.max(1) as f32;
    let boundaries = [
        0.0,
        (count / 3) as f32 * scale,
        (count * 2 / 3) as f32 * scale,
        width,
    ];
    for (i, pair) in boundaries.windows(2).enumerate() {
        shape = shape.fill(
            vec![
                [pair[0], 0.0],
                [pair[1], 0.0],
                [pair[1], height],
                [pair[0], height],
            ],
            color.alpha(if i == 1 { 0.06 } else { 0.035 }),
        );
    }
    shape
}

pub fn score(cp: i32) -> String {
    if cp.unsigned_abs() >= 90_000 {
        format!(
            "{}M{}",
            if cp >= 0 { "+" } else { "−" },
            100_000_u32.saturating_sub(cp.unsigned_abs())
        )
    } else {
        review_data::eval(cp)
    }
}

pub fn meter(s: &State, _k: &Kit, size: f32) -> El {
    let analysis = s.position_analysis();
    let value = analysis.map_or(50.0, |p| p.win_pct as f32);
    let label = analysis.map_or_else(|| "—".into(), |p| score(p.cp));
    const PAD: f32 = 1.0;
    const W: f32 = 14.0;
    let track_w = W - PAD * 2.0;
    let track_h = size - PAD * 2.0;
    let white_h = track_h * value.clamp(0.0, 100.0) / 100.0;
    let white_top = if s.flipped { 0.0 } else { track_h - white_h };
    let marker_top = if s.flipped {
        white_h - 1.0
    } else {
        track_h - white_h - 1.0
    };
    let marker_top = marker_top.clamp(0.0, track_h - 2.0);
    let dark = s.preferences.appearance == Appearance::Dark;
    let shell_top = if dark { "#303033" } else { "#eeeeef" };
    let shell_bottom = if dark { "#202022" } else { "#fafafa" };
    let shell_border = if dark { "#414145" } else { "#d4d4d8" };
    let meter = el()
        .key("evaluation-meter")
        .role("meter")
        .label(if analysis.is_some() {
            format!("Avaliação {label}")
        } else {
            "Avaliação indisponível".to_string()
        });
    meter
        .size(W, size)
        .radius(4.0)
        .gradient(
            rune::color::hex(shell_top).unwrap(),
            rune::color::hex(shell_bottom).unwrap(),
        )
        .border(1.0, rune::color::hex(shell_border).unwrap())
        .child(
            el().size(track_w, track_h)
                .absolute(PAD, PAD)
                .linear_gradient(
                    std::f32::consts::FRAC_PI_2,
                    [
                        (0.0, rune::color::hex("#18181b").unwrap()),
                        (1.0, rune::color::hex("#29292e").unwrap()),
                    ],
                )
                .clip_rounded(3.0)
                .child(
                    el().size(track_w, white_h)
                        .absolute(0.0, white_top)
                        .linear_gradient(
                            std::f32::consts::FRAC_PI_2,
                            [
                                (0.0, rune::color::hex("#e1e1de").unwrap()),
                                (1.0, rune::color::hex("#f5f5f4").unwrap()),
                            ],
                        )
                        .a11y_hidden(),
                )
                .child(
                    el().size(6.0, 1.0)
                        .absolute((track_w - 6.0) / 2.0, track_h / 2.0 - 0.5)
                        .fill(rune::color::hex("#8c8c8c").unwrap().alpha(0.4))
                        .a11y_hidden(),
                )
                .child(
                    el().size(track_w, 2.0)
                        .absolute(0.0, marker_top)
                        .fill(rune::color::hex("#a1a19b").unwrap())
                        .a11y_hidden(),
                )
                .child(
                    el().size(track_w, 1.0)
                        .absolute(0.0, (marker_top - 1.0).max(0.0))
                        .fill(Color::WHITE.alpha(0.25))
                        .a11y_hidden(),
                )
                .child(
                    el().size(track_w, 1.0)
                        .absolute(0.0, (marker_top + 2.0).min(track_h - 1.0))
                        .fill(Color::BLACK.alpha(0.20))
                        .a11y_hidden(),
                ),
        )
}

pub fn chart(s: &State, k: &Kit) -> El {
    const W: f32 = 320.0;
    const H: f32 = 124.0;
    let count = s.game.moves.len();
    let points: Vec<[f32; 2]> = (0..=count)
        .map(|i| {
            [
                i as f32 / count.max(1) as f32 * W,
                H * (1.0 - {
                    s.real_result
                        .as_ref()
                        .and_then(|r| r.positions.get(i))
                        .map_or(50.0, |p| p.win_pct as f32)
                } / 100.0),
            ]
        })
        .collect();
    let parts = split_midline(&smooth(&points), H / 2.0);
    let dark = s.preferences.appearance == Appearance::Dark;
    let white_line = if dark {
        oklch(0.967, 0.001, 286.4)
    } else {
        oklch(0.552, 0.016, 285.9)
    };
    let black_line = if dark {
        oklch(0.705, 0.015, 286.1)
    } else {
        oklch(0.274, 0.006, 286.0)
    };
    let white_fill = Color::WHITE.alpha(if dark { 0.14 } else { 0.72 });
    let black_fill = if dark {
        Color::BLACK.alpha(0.30)
    } else {
        black_line.alpha(0.12)
    };
    let x = s.ply as f32 / count.max(1) as f32 * W;
    let pct = {
        s.real_result
            .as_ref()
            .and_then(|r| r.positions.get(s.ply))
            .map_or(50.0, |p| p.win_pct as f32)
    };
    let y = H * (1.0 - pct / 100.0);
    let mut shape = phases(Shape::new(), count, W, H, k.c.text_base);
    for y in [H / 4.0, H * 3.0 / 4.0] {
        shape = dashed(shape, [0.0, y], [W, y], k.c.border_base);
    }
    shape = shape.line(
        [0.0, H / 2.0],
        [W, H / 2.0],
        1.0,
        LineCap::Butt,
        k.c.border_base,
    );
    for boundary in [count / 3, count * 2 / 3] {
        let x = boundary as f32 / count.max(1) as f32 * W;
        shape = dashed(shape, [x, 0.0], [x, H], k.c.border_base);
    }
    for (above, line) in &parts {
        if line.len() >= 2 {
            let mut area = line.clone();
            area.push([line[line.len() - 1][0], H / 2.0]);
            area.push([line[0][0], H / 2.0]);
            shape = shape.fill(area, if *above { white_fill } else { black_fill });
        }
    }
    for (above, line) in parts {
        shape = shape.stroke(line, 2.0, if above { white_line } else { black_line });
    }
    let marker_x = x.clamp(9.0, W - 9.0);
    let marker_y = y.clamp(9.0, H - 9.0);
    let tip = circle_pts(marker_x, marker_y, 4.0);
    shape = shape
        .line([x, 0.0], [x, H], 1.0, LineCap::Butt, k.c.bg_accent)
        .fill(
            circle_pts(marker_x, marker_y, 9.0),
            k.c.text_base.alpha(0.08),
        )
        .fill(tip.clone(), k.c.bg_base)
        .stroke_with(
            tip,
            true,
            1.75,
            LineCap::Round,
            if y <= H / 2.0 { white_line } else { black_line },
        )
        .view_box(W, H);
    let legend = |label, fill| {
        row()
            .center_y()
            .gap(6.0)
            .child(
                el().size(10.0, 10.0)
                    .radius(2.0)
                    .fill(fill)
                    .border(1.0, k.c.border_strong)
                    .a11y_hidden(),
            )
            .child(ui::label(label, 12.0, k.c.text_muted))
    };
    col()
        .key("evaluation-chart-card")
        .gap(12.0)
        .w_full()
        .padding(16.0)
        .radius(12.0)
        .border(1.0, k.c.border_base)
        .fill(k.c.bg_base)
        .child(
            row()
                .justify_between()
                .center_y()
                .child(
                    row()
                        .gap(12.0)
                        .center_y()
                        .child(legend("Brancas", Color::WHITE))
                        .child(legend("Pretas", Color::BLACK)),
                )
                .child(ui::mono(
                    {
                        s.real_result
                            .as_ref()
                            .and_then(|r| r.positions.get(s.ply))
                            .map_or_else(|| "—".into(), |p| review_data::eval(p.cp))
                    },
                    13.0,
                    k.c.text_base,
                )),
        )
        .child(
            el().key("evaluation-graph")
                .role("slider")
                .label("Gráfico de avaliação · selecione um lance")
                .range_value(s.ply as f64, 0.0, count as f64)
                .range_step(1.0)
                .focusable(true)
                .w_full()
                .h(H)
                .radius(8.0)
                .clip_rounded(8.0)
                .border(1.0, k.c.border_base)
                .shape(shape)
                .on_step(|s: &mut State, steps, tx| {
                    s.go_to(
                        (s.ply as i64 + steps as i64).clamp(0, s.game.moves.len() as i64) as usize,
                    );
                    crate::reveal_move(s, tx);
                })
                .on_drag(|s: &mut State, d, tx| {
                    if matches!(d.phase, Phase::Down | Phase::Move) {
                        s.go_to(
                            (d.x / d.w.max(1.0) * s.game.moves.len() as f32)
                                .round()
                                .clamp(0.0, s.game.moves.len() as f32)
                                as usize,
                        );
                        crate::reveal_move(s, tx);
                    }
                }),
        )
        .child(
            row()
                .justify_between()
                .center_y()
                .child(ui::label("Início", 12.0, k.c.text_muted))
                .child(ui::label("Fim", 12.0, k.c.text_muted)),
        )
}

#[cfg(test)]
mod tests {
    use super::{score, smooth};

    #[test]
    fn score_distinguishes_advantage_and_mate() {
        assert_eq!(score(142), "+1.42");
        assert_eq!(score(-142), "-1.42");
        assert_eq!(score(99_997), "+M3");
        assert_eq!(score(-99_997), "−M3");
    }

    #[test]
    fn smoothed_curve_passes_through_each_evaluation() {
        let points = [[0.0, 50.0], [10.0, 5.0], [20.0, 90.0]];
        let curve = smooth(&points);
        for point in points {
            assert!(curve.contains(&point), "missing evaluation {point:?}");
        }
    }
}
