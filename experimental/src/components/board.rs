//! Board and player tags. Position data comes from the validated PGN preview.
pub(crate) mod annotations;

use super::{pieces, review_feedback, ui::label};
use crate::{review_data, state::State, variation};
use annotations::{Annotation, Brush};
use rune::{El, LineCap, Phase, Shape, col, el, icon, oklch, row};
use rune_kit::Kit;
use shakmaty::Position;
pub fn view(s: &State, k: &Kit, size: f32) -> El {
    let square_size = size / 8.0;
    let pieces = if s.active_variation.is_some() {
        variation::board(s.position())
    } else {
        s.game.board(s.ply)
    };
    let last = s.last_move();
    let targets: Vec<usize> = s
        .position()
        .legal_moves()
        .into_iter()
        .filter(|m| {
            s.selected_square
                .is_some_and(|source| m.from().is_some_and(|from| from.to_usize() ^ 56 == source))
        })
        .map(|m| review_data::squares(m).1)
        .collect();
    let mut board = col()
        .key("chessboard")
        .role("group")
        .label("Tabuleiro de xadrez")
        .size(size, size);
    for rank in 0..8 {
        let mut line = row().h(square_size);
        for file in 0..8 {
            let index = if s.flipped {
                63 - rank * 8 - file
            } else {
                rank * 8 + file
            };
            let piece = pieces[index];
            let (piece_icon, piece_name) = match piece {
                'K' => (
                    Some((&pieces::KING_BODY, &pieces::KING_LINES, true)),
                    "Rei branco",
                ),
                'Q' => (
                    Some((&pieces::QUEEN_BODY, &pieces::QUEEN_LINES, true)),
                    "Dama branca",
                ),
                'R' => (
                    Some((&pieces::ROOK_BODY, &pieces::ROOK_LINES, true)),
                    "Torre branca",
                ),
                'B' => (
                    Some((&pieces::BISHOP_BODY, &pieces::BISHOP_LINES, true)),
                    "Bispo branco",
                ),
                'N' => (
                    Some((&pieces::KNIGHT_BODY, &pieces::KNIGHT_LINES, true)),
                    "Cavalo branco",
                ),
                'P' => (
                    Some((&pieces::PAWN_BODY, &pieces::PAWN_LINES, true)),
                    "Peão branco",
                ),
                'k' => (
                    Some((&pieces::KING_BODY, &pieces::KING_LINES, false)),
                    "Rei preto",
                ),
                'q' => (
                    Some((&pieces::QUEEN_BODY, &pieces::QUEEN_LINES, false)),
                    "Dama preta",
                ),
                'r' => (
                    Some((&pieces::ROOK_BODY, &pieces::ROOK_LINES, false)),
                    "Torre preta",
                ),
                'b' => (
                    Some((&pieces::BISHOP_BODY, &pieces::BISHOP_LINES, false)),
                    "Bispo preto",
                ),
                'n' => (
                    Some((&pieces::KNIGHT_BODY, &pieces::KNIGHT_LINES, false)),
                    "Cavalo preto",
                ),
                'p' => (
                    Some((&pieces::PAWN_BODY, &pieces::PAWN_LINES, false)),
                    "Peão preto",
                ),
                _ => (None, "vazia"),
            };
            let square = format!("{}{}", (b'a' + (index % 8) as u8) as char, 8 - index / 8);
            let mut tile = el()
                .key(format!("square-{square}"))
                .label(format!(
                    "{square} {piece_name}{}{}",
                    if s.selected_square == Some(index) {
                        ", selecionada"
                    } else {
                        ""
                    },
                    if targets.contains(&index) {
                        ", destino legal"
                    } else {
                        ""
                    }
                ))
                .size(square_size, square_size)
                .center()
                .fill(if (rank + file) % 2 == 0 {
                    oklch(0.86, 0.025, 88.0)
                } else {
                    oklch(0.59, 0.055, 125.0)
                });
            tile = tile
                .role("button")
                .focusable(true)
                .on_secondary_press(move |s: &mut State, _, tx| {
                    let mods = tx.mods();
                    s.board_annotations.toggle(Annotation {
                        from: index,
                        to: index,
                        brush: Brush::from_modifiers(
                            mods.shift || mods.ctrl,
                            mods.alt || mods.logo,
                        ),
                    });
                })
                .on_press_tx(move |s: &mut State, tx| {
                    if s.board_annotations.drawing() {
                        s.board_annotations.select_square(index);
                    } else {
                        s.click_square(index);
                        crate::reveal_move(s, tx);
                    }
                })
                .on_drag(move |s: &mut State, d, tx| match d.phase {
                    Phase::Up
                        if !s.board_annotations.drawing()
                            && !d.cancelled()
                            && d.dx.hypot(d.dy) > 6.0 =>
                    {
                        let to_file = file as i32 + (d.x / square_size).floor() as i32;
                        let to_rank = rank as i32 + (d.y / square_size).floor() as i32;
                        if (0..8).contains(&to_file) && (0..8).contains(&to_rank) {
                            let target = to_rank as usize * 8 + to_file as usize;
                            s.click_square(index);
                            s.click_square(if s.flipped { 63 - target } else { target });
                            crate::reveal_move(s, tx);
                        }
                    }
                    _ => {}
                });
            if last.is_some_and(|(from, to)| from == index || to == index) {
                tile = tile.fill(oklch(0.75, 0.11, 92.0));
            }
            if s.selected_square == Some(index) || s.board_annotations.origin() == Some(index) {
                tile = tile.border(3.0, k.c.bg_accent);
            }
            if let Some((body, lines, white)) = piece_icon {
                let sz = (square_size * 0.78).round();
                let (fill, stroke) = if white {
                    (
                        rune::color::hex("#f5f1e8").unwrap(),
                        rune::color::hex("#292722").unwrap(),
                    )
                } else {
                    (
                        rune::color::hex("#302e2a").unwrap(),
                        rune::color::hex("#d3c9b7").unwrap(),
                    )
                };
                tile = tile.child(
                    el().size(sz, sz)
                        .child(icon(body, sz).color(fill).absolute(0.0, 0.0))
                        .child(icon(lines, sz).color(stroke).absolute(0.0, 0.0))
                        .inert()
                        .a11y_hidden(),
                );
            }
            if targets.contains(&index) {
                tile = tile.child(
                    el().size(square_size * 0.2, square_size * 0.2)
                        .radius(square_size * 0.1)
                        .absolute(square_size * 0.4, square_size * 0.4)
                        .fill(k.c.bg_accent.alpha(0.7))
                        .inert()
                        .a11y_hidden(),
                );
            }
            if s.preferences.feedback
                && last.is_some_and(|(_, to)| to == index)
                && let Some(class) = s.displayed_classification()
            {
                tile = tile.child(
                    review_feedback::badge(class)
                        .absolute(square_size - 22.0, 0.0)
                        .inert(),
                );
            }
            line = line.child(tile);
        }
        board = board.child(line);
    }
    for (i, annotation) in s.board_annotations.shapes().iter().enumerate() {
        let center = |index: usize| {
            let index = if s.flipped { 63 - index } else { index };
            [
                (index % 8) as f32 * square_size + square_size / 2.0,
                (index / 8) as f32 * square_size + square_size / 2.0,
            ]
        };
        let start = center(annotation.from);
        let end = center(annotation.to);
        let color = annotation.brush.color();
        let shape = if annotation.from == annotation.to {
            let radius = square_size * 0.36;
            let points: Vec<_> = (0..32)
                .map(|step| {
                    let angle = step as f32 * std::f32::consts::TAU / 32.0;
                    [
                        start[0] + radius * angle.cos(),
                        start[1] + radius * angle.sin(),
                    ]
                })
                .collect();
            Shape::new().stroke_with(points, true, square_size * 0.08, LineCap::Round, color)
        } else {
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let len = dx.hypot(dy);
            let (ux, uy) = (dx / len, dy / len);
            let tip = square_size * 0.26;
            let end = [
                end[0] - ux * square_size * 0.18,
                end[1] - uy * square_size * 0.18,
            ];
            Shape::new()
                .line(start, end, square_size * 0.1, LineCap::Round, color)
                .fill(
                    vec![
                        end,
                        [
                            end[0] - ux * tip - uy * tip * 0.6,
                            end[1] - uy * tip + ux * tip * 0.6,
                        ],
                        [
                            end[0] - ux * tip + uy * tip * 0.6,
                            end[1] - uy * tip - ux * tip * 0.6,
                        ],
                    ],
                    color,
                )
        };
        board = board.child(
            el().key(format!("user-annotation-{i}"))
                .shape(shape)
                .size(size, size)
                .absolute(0.0, 0.0)
                .inert()
                .a11y_hidden(),
        );
    }
    if s.preferences.live_engine {
        let colors = [
            k.c.bg_accent,
            oklch(0.72, 0.15, 155.0),
            oklch(0.78, 0.14, 80.0),
        ];
        let candidates = s.candidates();
        for (i, line) in candidates
            .iter()
            .take(s.preferences.lines.min(colors.len()))
            .enumerate()
            .rev()
        {
            let Some(m) = line.moves.first().copied() else {
                continue;
            };
            let color = colors[i];
            let (from, to) = review_data::squares(m);
            let center = |index: usize| {
                let index = if s.flipped { 63 - index } else { index };
                [
                    (index % 8) as f32 * square_size + square_size / 2.0,
                    (index / 8) as f32 * square_size + square_size / 2.0,
                ]
            };
            let start = center(from);
            let end = center(to);
            let dx = end[0] - start[0];
            let dy = end[1] - start[1];
            let len = dx.hypot(dy).max(1.0);
            let ux = dx / len;
            let uy = dy / len;
            let shared = candidates
                .iter()
                .take(i)
                .filter(|candidate| {
                    candidate
                        .moves
                        .first()
                        .is_some_and(|m| review_data::squares(*m).0 == from)
                })
                .count();
            let offset = shared as f32 * square_size * 0.12;
            let start = [
                start[0] + ux * square_size * 0.24 - uy * offset,
                start[1] + uy * square_size * 0.24 + ux * offset,
            ];
            let end = [
                end[0] - ux * square_size * 0.18 - uy * offset,
                end[1] - uy * square_size * 0.18 + ux * offset,
            ];
            let tip = square_size * 0.26;
            let arrow = Shape::new()
                .line(
                    start,
                    end,
                    square_size * 0.1,
                    LineCap::Round,
                    color.alpha(0.75),
                )
                .fill(
                    vec![
                        end,
                        [
                            end[0] - ux * tip - uy * tip * 0.6,
                            end[1] - uy * tip + ux * tip * 0.6,
                        ],
                        [
                            end[0] - ux * tip + uy * tip * 0.6,
                            end[1] - uy * tip - ux * tip * 0.6,
                        ],
                    ],
                    color.alpha(0.9),
                );
            board = board.child(
                el().key(format!("candidate-arrow-{i}"))
                    .shape(arrow)
                    .size(size, size)
                    .absolute(0.0, 0.0)
                    .inert()
                    .a11y_hidden(),
            );
        }
    }
    col()
        .gap(7.0)
        .child(board)
        .child((0..8).fold(row().w(size), |line, file| {
            let file = if s.flipped { 7 - file } else { file };
            line.child(el().w(square_size).center().child(label(
                ((b'a' + file) as char).to_string(),
                10.0,
                k.c.text_muted,
            )))
        }))
}
pub fn player(
    name: &str,
    side: &str,
    image: Option<&rune::Image>,
    rating: &Option<String>,
    k: &Kit,
    size: f32,
    compact: bool,
) -> El {
    let avatar_size = if compact { 32.0 } else { 40.0 };
    let fallback = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    let mut avatar = el()
        .size(avatar_size, avatar_size)
        .radius(if compact { 6.0 } else { 8.0 })
        .fill(k.c.layer_01)
        .center();
    if let Some(image) = image.filter(|image| image.is_ready() && image.error().is_none()) {
        avatar = avatar.pixels_fit(image.pixels(), rune::SurfaceFit::Fill);
    } else {
        avatar = avatar.child(label(fallback, 11.0, k.c.text_base));
    }
    row()
        .w(size)
        .h(avatar_size)
        .gap(10.0)
        .center_y()
        .child(avatar.a11y_hidden())
        .child(
            label(name, 13.0, k.c.text_base)
                .truncate()
                .max_w((size - avatar_size - 132.0).max(40.0)),
        )
        .child(
            rating
                .as_ref()
                .map_or_else(el, |rating| label(rating, 12.0, k.c.text_base)),
        )
        .child(el().grow(1.0))
        .child(label(side, 10.0, k.c.text_muted))
}
