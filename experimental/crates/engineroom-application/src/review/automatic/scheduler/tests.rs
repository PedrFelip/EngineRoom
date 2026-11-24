use super::*;
use crate::review::{core, types::RawLine};

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
    let mut game = core::extract("1. Nf3 Nf6 2. Ng1 Ng8").unwrap();
    game.fens[4] = game.fens[0].replace(" 0 1", " 0 3");
    game.positions[4] = core::position(&game.fens[4]).unwrap();
    game.fens[2] = game.fens[0].replace(" 0 1", " 1 1");
    game.positions[2] = core::position(&game.fens[2]).unwrap();
    let mut scheduler = Scheduler::new(&game);
    scheduler.prepare();
    let first = scheduler.ply_id(0).unwrap();
    let different_clock = scheduler.ply_id(2).unwrap();
    assert_eq!(scheduler.ply_id(4), Some(first));
    assert_eq!(scheduler.plies(first).collect::<Vec<_>>(), [0, 4]);
    assert_ne!(first, different_clock);
    assert_eq!(
        scheduler.intern_live(&game.fens[0].replace(" 0 1", " 0 9"), || 20),
        first
    );
    assert_ne!(
        scheduler.intern_live(&game.fens[0].replace(" 0 1", " 2 1"), || 20),
        first
    );

    let mut queue = VecDeque::from([4, 2, 3]);
    scheduler.reserve(first);
    assert_eq!(scheduler.take_triage(&mut queue), Some(2));
    assert_eq!(scheduler.take_triage(&mut queue), Some(3));
    assert_eq!(scheduler.take_triage(&mut queue), None);

    assert_eq!(
        scheduler.complete(first, Demand::time(180, 1), &evidence(&game.fens[0], &[12])),
        1
    );
    assert!(scheduler.lookup(first, Demand::time(180, 1)).is_some());
    assert!(
        scheduler
            .lookup(different_clock, Demand::time(180, 1))
            .is_none()
    );
    assert!(scheduler.lookup(first, Demand::time(500, 1)).is_none());
    assert_eq!(scheduler.take_triage(&mut queue), Some(4));
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
    let mut scheduler = Scheduler::new(&game);
    scheduler.prepare();
    let id = scheduler.ply_id(0).unwrap();

    let mut missing_principal = evidence(fen, &[15]);
    missing_principal.lines[0].multipv = 2;
    scheduler.reserve(id);
    assert_eq!(
        scheduler.complete(id, Demand::time(500, 3), &missing_principal),
        0
    );
    assert!(scheduler.available(id));
    assert!(scheduler.lookup(id, Demand::time(500, 1)).is_none());

    scheduler.reserve(id);
    assert_eq!(
        scheduler.complete(id, Demand::time(500, 3), &evidence(fen, &[19, 17])),
        2
    );
    assert_eq!(
        scheduler
            .lookup(id, Demand::time(400, 1))
            .unwrap()
            .lines
            .len(),
        1
    );
    assert!(scheduler.lookup(id, Demand::time(400, 3)).is_none());
    assert!(scheduler.lookup(id, depth(17, 2)).is_some());
    assert!(scheduler.lookup(id, depth(18, 2)).is_none());
    scheduler.reserve(id);
    assert_eq!(
        scheduler.complete(id, depth(24, 3), &evidence(fen, &[25, 24, 24])),
        3
    );
    assert!(scheduler.lookup(id, depth(24, 3)).is_some());
    assert!(scheduler.lookup(id, Demand::time(1, 3)).is_none());
}

#[test]
fn arena_keeps_first_covering_evidence_and_uses_seeded_move_count() {
    let game = core::extract("[FEN \"8/7k/5KQ1/8/8/8/8/8 b - - 0 1\"]\n1... Kh8").unwrap();
    let fen = &game.fens[0];
    let mut scheduler = Scheduler::new(&game);
    assert!(scheduler.index.is_none());
    scheduler.prepare();
    let id = scheduler.ply_id(0).unwrap();
    assert_eq!(scheduler.index.as_ref().unwrap().legal_moves[id.index()], 1);
    scheduler.reserve(id);
    assert_eq!(
        scheduler.complete(id, Demand::time(500, 1), &evidence(fen, &[12])),
        1
    );
    scheduler.reserve(id);
    assert_eq!(
        scheduler.complete(id, Demand::time(500, 3), &evidence(fen, &[20])),
        3
    );
    let hit = scheduler.lookup(id, Demand::time(500, 3)).unwrap();
    assert_eq!(hit.depth, 12);
    assert_eq!(hit.lines.len(), 1);
}

#[test]
fn activity_bitset_keeps_high_indices_independent() {
    let mut active = BitSet::default();
    for index in [0, 63, 64, 129] {
        active.insert(index);
    }
    active.remove(64);
    assert!(!active.contains(64));
    assert!(!active.contains(192));
    assert!([0, 63, 129].into_iter().all(|index| active.contains(index)));
}
