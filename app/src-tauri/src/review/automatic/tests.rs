use super::*;

#[test]
fn refinement_keeps_baseline_roots_and_played_move_without_duplicates() {
    let mut raw = core::terminal_raw("", 0);
    raw.pv = vec!["e2e4".into(), "e7e5".into()];
    raw.lines[0].pv = raw.pv.clone();
    let mut alternative = raw.lines[0].clone();
    alternative.multipv = 2;
    alternative.pv = vec!["d2d4".into(), "d7d5".into()];
    raw.lines.push(alternative);
    assert_eq!(
        refinement_candidates(&raw, Some("g1f3")),
        vec!["e2e4", "d2d4", "g1f3"]
    );
    assert_eq!(
        refinement_candidates(&raw, Some("e2e4")),
        vec!["e2e4", "d2d4"]
    );
}
