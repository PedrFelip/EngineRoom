//! Position phase detection and monotonic game phase progression.
use crate::review::types::Phase;

fn region(y: i32, w: usize, b: usize) -> i32 {
    match (w, b) {
        (0, 1) => 1 + y,
        (0, 2) if y < 6 => 2 + 6 - y,
        (0, 3 | 4) if y < 7 => 3 + 7 - y,
        (1, 0) => 1 + 8 - y,
        (1, 1) => 5 + (4 - y).abs(),
        (1, 2) => 4 + 7 - y,
        (1, 3) => 5 + 7 - y,
        (2, 0) if y > 2 => 2 + y - 2,
        (2, 1) => 4 + y - 1,
        (2, 2) => 7,
        (3, 0) if y > 1 => 3 + y - 1,
        (3, 1) => 5 + y - 1,
        (4, 0) if y > 1 => 3 + y - 1,
        _ => 0,
    }
}
pub fn phase(fen: &str) -> Phase {
    let mut split_ranks = fen.split_whitespace().next().unwrap_or("").split('/');
    let mut ranks = [""; 8];
    for rank in &mut ranks {
        let Some(next) = split_ranks.next() else {
            return Phase::Opening;
        };
        *rank = next;
    }
    if split_ranks.next().is_some() {
        return Phase::Opening;
    }
    let pieces = ranks
        .iter()
        .flat_map(|s| s.chars())
        .filter(|c| "nbrqNBRQ".contains(*c))
        .count();
    if pieces <= 6 {
        return Phase::Endgame;
    }
    if pieces <= 10
        || ranks[7].chars().filter(|c| c.is_ascii_uppercase()).count() < 4
        || ranks[0].chars().filter(|c| c.is_ascii_lowercase()).count() < 4
    {
        return Phase::Middlegame;
    }
    let mut white = 0_u64;
    let mut black = 0_u64;
    for (i, rank) in ranks.iter().enumerate() {
        let mut file = 0;
        for c in rank.chars() {
            if let Some(n) = c.to_digit(10) {
                file += n as usize;
            } else if file < 8 {
                let square = 1_u64 << ((7 - i) * 8 + file);
                if c.is_ascii_uppercase() {
                    white |= square;
                } else {
                    black |= square;
                }
                file += 1;
            }
        }
    }
    let mut mixed = 0;
    for y in 0..7 {
        for x in 0..7 {
            // The low two bits of each of two adjacent ranks form a 2×2 region.
            let region_mask = 0x0303_u64 << (y * 8 + x);
            let w = (white & region_mask).count_ones() as usize;
            let b = (black & region_mask).count_ones() as usize;
            mixed += region(y + 1, w, b);
        }
    }
    if mixed > 150 {
        Phase::Middlegame
    } else {
        Phase::Opening
    }
}
pub fn phases(fens: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<Phase> {
    let mut current = Phase::Opening;
    fens.into_iter()
        .map(|f| {
            let next = phase(f.as_ref());
            if next as u8 > current as u8 {
                current = next;
            }
            current
        })
        .collect()
}
