use super::*;
use crate::review::types::RawLine;

fn evidence(fen: &str, depths: &[u32]) -> RawPosition {
    RawPosition {
        fen: fen.into(),
        depth: depths[0],
        cp: 0,
        pv: vec![],
        lines: depths
            .iter()
            .enumerate()
            .map(|(i, depth)| RawLine {
                multipv: i as u32 + 1,
                depth: Some(*depth),
                unstable: false,
                cp: 0,
                pv: vec![],
                san: None,
            })
            .collect(),
    }
}

#[test]
fn exact_fen_reservation_skips_blocked_work_and_resumes_stronger_demand() {
    let game = core::extract("1. e4 e5").unwrap();
    let first = &game.fens[0];
    let different_clock = first.replace(" 0 1", " 1 1");
    let fens = vec![
        first.clone(),
        first.clone(),
        different_clock,
        game.fens[1].clone(),
    ];
    let mut scheduler = Scheduler::default();
    let mut queue = VecDeque::from([1, 2, 3]);
    let mut blocks = vec![VecDeque::new(); 2];
    scheduler.reserve(first);
    assert_eq!(
        scheduler.take_triage(0, &mut queue, &mut blocks, &fens),
        Some(2)
    );
    assert_eq!(
        scheduler.take_triage(1, &mut queue, &mut blocks, &fens),
        Some(3)
    );
    assert_eq!(
        scheduler.take_triage(1, &mut queue, &mut blocks, &fens),
        None
    );
    scheduler.complete(Demand::time(180, 1), &evidence(first, &[12]));
    assert!(scheduler.lookup(first, Demand::time(180, 1)).is_some());
    assert!(scheduler.lookup(&fens[2], Demand::time(180, 1)).is_none());
    assert!(scheduler.lookup(first, Demand::time(500, 1)).is_none());
    assert_eq!(
        scheduler.take_triage(1, &mut queue, &mut blocks, &fens),
        Some(1)
    );
}

#[test]
fn coverage_uses_actual_contiguous_pvs_and_sql_budget_semantics() {
    let game = core::extract("1. e4").unwrap();
    let fen = &game.fens[0];
    let depth = |value, multipv| Demand {
        mode: Mode::Depth,
        value,
        multipv,
    };
    let mut scheduler = Scheduler::default();
    scheduler.reserve(fen);
    scheduler.complete(Demand::time(500, 3), &evidence(fen, &[19, 17]));
    assert_eq!(
        scheduler
            .lookup(fen, Demand::time(400, 1))
            .unwrap()
            .lines
            .len(),
        1
    );
    assert!(scheduler.lookup(fen, Demand::time(400, 3)).is_none());
    assert!(scheduler.lookup(fen, depth(17, 2)).is_some());
    assert!(scheduler.lookup(fen, depth(18, 2)).is_none());
    scheduler.reserve(fen);
    scheduler.complete(depth(24, 3), &evidence(fen, &[25, 24, 24]));
    assert!(scheduler.lookup(fen, depth(24, 3)).is_some());
    assert!(scheduler.lookup(fen, Demand::time(1, 3)).is_none());
}
