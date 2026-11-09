//! Board and player tags. Position data comes from the validated PGN preview.
use super::{pieces, review_feedback, ui::label};
use crate::{
    review_data,
    state::{ReviewTab, State},
    variation,
};
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
                .label(format!("{square} {piece_name}"))
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
                .on_drag(move |s: &mut State, d, _| match d.phase {
                    Phase::Down => s.click_square(index),
                    Phase::Up if !d.cancelled() && d.dx.hypot(d.dy) > 6.0 => {
                        let to_file = file as i32 + (d.x / square_size).floor() as i32;
                        let to_rank = rank as i32 + (d.y / square_size).floor() as i32;
                        if (0..8).contains(&to_file) && (0..8).contains(&to_rank) {
                            let target = to_rank as usize * 8 + to_file as usize;
                            s.click_square(if s.flipped { 63 - target } else { target });
                        }
                    }
                    _ => {}
                });
            if last.is_some_and(|(from, to)| from == index || to == index) {
                tile = tile.fill(oklch(0.75, 0.11, 92.0));
            }
            if s.selected_square == Some(index) {
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
                && s.active_variation.is_none()
                && last.is_some_and(|(_, to)| to == index)
                && let Some(class) = s.classification(s.displayed_ply())
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
    if s.preferences.live_engine
        && s.review_tab == ReviewTab::Analysis
        && let Some(m) = s
            .candidates()
            .get(s.selected_candidate)
            .and_then(|line| line.moves.first())
            .copied()
    {
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
        let tip = square_size * 0.26;
        let arrow = Shape::new()
            .line(
                start,
                end,
                square_size * 0.1,
                LineCap::Round,
                k.c.bg_accent.alpha(0.8),
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
                k.c.bg_accent.alpha(0.9),
            );
        board = board.child(
            el().key("candidate-arrow")
                .shape(arrow)
                .size(size, size)
                .absolute(0.0, 0.0)
                .inert()
                .a11y_hidden(),
        );
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
pub fn player(name: &str, side: &str, k: &Kit, size: f32) -> El {
    row()
        .w(size)
        .h(32.0)
        .gap(10.0)
        .center_y()
        .child(
            el().size(28.0, 28.0)
                .radius(6.0)
                .fill(k.c.layer_01)
                .center()
                .child(label(
                    if side == "Brancas" { "B" } else { "P" },
                    11.0,
                    k.c.text_base,
                )),
        )
        .child(label(name, 13.0, k.c.text_base).truncate().grow(1.0))
        .child(label(side, 10.0, k.c.text_muted))
}
