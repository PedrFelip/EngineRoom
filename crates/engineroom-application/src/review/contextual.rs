//! Mainline evidence uses a separate cache namespace from FEN-only analysis.
use super::{
    core::{Game, PositionContext},
    repository::{ContextualEvaluation, Repository},
    types::*,
};
use crate::db::mode::Mode;

pub(super) struct ContextualReview {
    signature: String,
    keys: Vec<String>,
    fens: Vec<String>,
}

impl ContextualReview {
    pub fn new(game: &Game, signature: &str, sizing: Option<(u32, u32)>) -> Self {
        Self::with_execution(game, signature, sizing, 1)
    }

    pub fn automatic(game: &Game, signature: &str, sizing: Option<(u32, u32)>) -> Self {
        let workers = super::pool::worker_count(sizing, game.fens.len());
        Self::with_execution(game, signature, sizing, workers)
    }

    fn with_execution(
        game: &Game,
        signature: &str,
        sizing: Option<(u32, u32)>,
        workers: usize,
    ) -> Self {
        let mut moves = String::new();
        let keys = (0..game.fens.len())
            .map(|ply| {
                if ply > 0 {
                    if ply > 1 {
                        moves.push(' ');
                    }
                    moves.push_str(&game.moves[ply - 1].uci);
                }
                // Full payload is the key: matching the key verifies the context.
                serde_json::json!([
                    "mainline-unrestricted-v1",
                    signature,
                    sizing,
                    workers,
                    game.fens[0],
                    moves
                ])
                .to_string()
            })
            .collect();
        Self {
            signature: signature.into(),
            keys,
            fens: game.fens.clone(),
        }
    }

    pub fn position(game: &Game, ply: usize) -> Result<PositionContext> {
        PositionContext::from_game(game, ply)
    }

    pub async fn lookup(
        &self,
        repository: &dyn Repository,
        plies: &[usize],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Result<Vec<Option<ContextualEvaluation>>> {
        let keys: Vec<_> = plies.iter().map(|&ply| self.keys[ply].clone()).collect();
        let mut results = repository
            .lookup_context(&keys, mode, value, multipv)
            .await?;
        if results.len() != keys.len() {
            return Err(ReviewError::new(
                ReviewErrorCode::Cache,
                "cache.context.lookup",
                "Quantidade de avaliações inválida.",
            ));
        }
        for ((hit, key), ply) in results.iter_mut().zip(keys).zip(plies) {
            if hit.as_ref().is_some_and(|hit| {
                hit.key != key
                    || hit.raw.fen != self.fens[*ply]
                    || hit.evidence.engine_build.as_deref() != Some(&self.signature)
                    || hit.evidence.history_ply != Some(*ply)
                    || hit.evidence.scope != EvidenceScope::Mainline
                    || hit.evidence.origin != EvidenceOrigin::Engine
                    || hit.evidence.completion != EvidenceCompletion::BestMove
            }) {
                *hit = None;
            }
        }
        Ok(results)
    }

    pub fn entry(
        &self,
        ply: usize,
        raw: &RawPosition,
        engine: Option<String>,
        budget: EvidenceBudget,
    ) -> ContextualEvaluation {
        let mut evidence = PositionEvidence::from_raw(
            raw,
            Some(ply),
            EvidenceOrigin::Engine,
            engine,
            Some(budget),
        );
        evidence.engine_build = Some(self.signature.clone());
        ContextualEvaluation {
            key: self.keys[ply].clone(),
            raw: raw.clone(),
            evidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::mode::Mode;
    use crate::review::core::extract;
    use crate::review::engine::Task;
    use std::sync::Mutex;

    const START: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    /// Only the piece placement, so transpositions that differ in clocks or
    /// en-passant squares still compare equal.
    fn board(fen: &str) -> &str {
        fen.split_whitespace().next().unwrap()
    }

    fn raw(fen: &str) -> RawPosition {
        RawPosition {
            fen: fen.into(),
            cp: 12,
            depth: 14,
            pv: vec!["e2e4".into()],
            lines: vec![RawLine {
                unstable: false,
                multipv: 1,
                cp: 12,
                pv: vec!["e2e4".into()],
                san: Some("e4".into()),
                depth: Some(14),
            }],
        }
    }

    /// Returns a configurable list of contextual hits and records the exact
    /// keys/budget `lookup` forwarded to the repository boundary.
    type CacheLookup = (Vec<String>, Mode, u32, u32);

    struct ContextRepo {
        hits: Vec<Option<ContextualEvaluation>>,
        seen: Mutex<Vec<CacheLookup>>,
        fail: bool,
    }

    impl ContextRepo {
        fn with_hits(hits: Vec<Option<ContextualEvaluation>>) -> Self {
            Self {
                hits,
                seen: Mutex::new(Vec::new()),
                fail: false,
            }
        }
    }

    impl Repository for ContextRepo {
        fn lookup<'a>(
            &'a self,
            fens: &'a [String],
            _: Mode,
            _: u32,
            _: u32,
        ) -> Task<'a, Result<Vec<Option<RawPosition>>>> {
            Box::pin(async move { Ok(vec![None; fens.len()]) })
        }

        fn put<'a>(
            &'a self,
            _: &'a [RawPosition],
            _: Mode,
            _: u32,
            _: u32,
        ) -> Task<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn save<'a>(&'a self, _: &'a ReviewConfig, _: &'a ReviewResult) -> Task<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn lookup_context<'a>(
            &'a self,
            keys: &'a [String],
            mode: Mode,
            value: u32,
            multipv: u32,
        ) -> Task<'a, Result<Vec<Option<ContextualEvaluation>>>> {
            Box::pin(async move {
                self.seen
                    .lock()
                    .unwrap()
                    .push((keys.to_vec(), mode, value, multipv));
                if self.fail {
                    return Err(ReviewError::new(
                        ReviewErrorCode::Cache,
                        "cache.context.lookup",
                        "boom",
                    ));
                }
                Ok(self.hits.clone())
            })
        }
    }

    #[test]
    fn keys_separate_different_known_histories_for_the_same_board() {
        // The starting position repeats after four knight moves: same board,
        // different known history, so it must not reuse the opening's evidence.
        let repeat = extract("1. Nf3 Nf6 2. Ng1 Ng8").unwrap();
        assert_eq!(repeat.fens.len(), 5);
        assert_eq!(repeat.fens[0], START);
        assert_eq!(board(&repeat.fens[0]), board(&repeat.fens[4]));
        let r = ContextualReview::new(&repeat, "sf19", None);
        assert_ne!(r.keys[0], r.keys[4]);

        // Two transpositions reach the same board through different move orders.
        let a = extract("1. Nf3 Nc6 2. Nc3 Nf6").unwrap();
        let b = extract("1. Nc3 Nf6 2. Nf3 Nc6").unwrap();
        assert_eq!(a.fens[4], b.fens[4]);
        let ka = ContextualReview::new(&a, "sf19", None);
        let kb = ContextualReview::new(&b, "sf19", None);
        assert_ne!(ka.keys[4], kb.keys[4]);
    }

    #[test]
    fn keys_bind_engine_signature_and_resource_sizing() {
        let game = extract("1. e4").unwrap();
        let a = ContextualReview::new(&game, "sf19-aaa", None);
        let b = ContextualReview::new(&game, "sf19-bbb", None);
        let sized = ContextualReview::new(&game, "sf19-aaa", Some((4, 64)));
        assert_ne!(
            a.keys[1], b.keys[1],
            "engine builds must not share evidence"
        );
        assert_ne!(a.keys[1], sized.keys[1], "sizing must not share evidence");
        assert_eq!(
            a.keys[1],
            ContextualReview::new(&game, "sf19-aaa", None).keys[1]
        );

        let value: serde_json::Value = serde_json::from_str(&a.keys[1]).unwrap();
        assert_eq!(value[0], serde_json::json!("mainline-unrestricted-v1"));
        assert_eq!(value[1], serde_json::json!("sf19-aaa"));
        assert_eq!(value[2], serde_json::Value::Null);
        assert_eq!(value[3], serde_json::json!(1));
        assert_eq!(value[4], serde_json::json!(game.fens[0]));
        assert_eq!(value[5], serde_json::json!("e2e4"));
        let sized_value: serde_json::Value = serde_json::from_str(&sized.keys[1]).unwrap();
        assert_eq!(sized_value[2], serde_json::json!([4, 64]));
    }

    #[test]
    fn automatic_keys_bind_worker_count_separately_from_manual_keys() {
        let game = extract("1. e4 e5 2. Nf3 Nc6 3. Bb5").unwrap();
        let manual = ContextualReview::new(&game, "sf19", Some((8, 128)));
        let automatic = ContextualReview::automatic(&game, "sf19", Some((8, 128)));
        assert_ne!(manual.keys[4], automatic.keys[4]);
        let value: serde_json::Value = serde_json::from_str(&automatic.keys[4]).unwrap();
        assert_eq!(value[3], serde_json::json!(3));
        let manual_value: serde_json::Value = serde_json::from_str(&manual.keys[4]).unwrap();
        assert_eq!(manual_value[3], serde_json::json!(1));
        // With a single worker both constructors describe the same execution.
        let single = ContextualReview::automatic(&game, "sf19", None);
        assert_eq!(
            single.keys[4],
            ContextualReview::new(&game, "sf19", None).keys[4]
        );
    }

    #[test]
    fn keys_encode_the_exact_uci_prefix_at_each_ply() {
        let game = extract("1. e4 e5 2. Nf3").unwrap();
        let review = ContextualReview::new(&game, "sf19", None);
        assert_eq!(review.keys.len(), game.fens.len());
        assert_eq!(review.keys.len(), game.moves.len() + 1);
        for ply in 0..game.fens.len() {
            let value: serde_json::Value = serde_json::from_str(&review.keys[ply]).unwrap();
            let expected = game.moves[..ply]
                .iter()
                .map(|m| m.uci.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            assert_eq!(value[5].as_str().unwrap(), expected, "ply {ply}");
            assert_eq!(value[4], serde_json::json!(game.fens[0]));
        }
    }

    #[test]
    fn entry_records_mainline_engine_evidence_with_the_review_signature() {
        let game = extract("1. e4 e5").unwrap();
        let review = ContextualReview::new(&game, "sf19-aaa", Some((2, 16)));
        let mut position = raw(&game.fens[2]);
        position.pv[0] = "g1f3".into();
        position.lines[0].pv[0] = "g1f3".into();
        let budget = EvidenceBudget {
            mode: Mode::Time,
            value: 250,
            multipv: 1,
        };
        let entry = review.entry(2, &position, Some("Stockfish 19".into()), budget);

        assert_eq!(entry.key, review.keys[2]);
        assert_eq!(
            serde_json::to_value(&entry.raw).unwrap(),
            serde_json::to_value(&position).unwrap()
        );
        assert_eq!(entry.evidence.engine_build.as_deref(), Some("sf19-aaa"));
        assert_eq!(entry.evidence.engine.as_deref(), Some("Stockfish 19"));
        assert_eq!(entry.evidence.history_ply, Some(2));
        assert_eq!(entry.evidence.scope, EvidenceScope::Mainline);
        assert_eq!(entry.evidence.origin, EvidenceOrigin::Engine);
        assert_eq!(entry.evidence.requested, Some(budget));
    }

    #[test]
    fn position_builds_the_replayable_mainline_context() {
        let game = extract("1. e4 e5 2. Nf3").unwrap();
        let context = ContextualReview::position(&game, 3).unwrap();
        assert_eq!(context, PositionContext::from_game(&game, 3).unwrap());
        assert_eq!(context.known_history, 3);
        assert_eq!(context.history_ply(), Some(3));
        assert!(ContextualReview::position(&game, 4).is_err());
    }

    #[tokio::test]
    async fn lookup_forwards_budget_and_accepts_a_matching_hit() {
        let game = extract("1. e4 e5 2. Nf3").unwrap();
        let review = ContextualReview::new(&game, "sf19", Some((2, 16)));
        let budget = EvidenceBudget {
            mode: Mode::Depth,
            value: 12,
            multipv: 1,
        };
        let hit = review.entry(3, &raw(&game.fens[3]), Some("sf".into()), budget);
        let repo = ContextRepo::with_hits(vec![Some(hit.clone())]);

        let results = review
            .lookup(&repo, &[3], Mode::Depth, 12, 1)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].as_ref().map(|e| e.key.as_str()),
            Some(review.keys[3].as_str())
        );
        let seen = repo.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].0, vec![review.keys[3].clone()]);
        assert_eq!(seen[0].1, Mode::Depth);
        assert_eq!((seen[0].2, seen[0].3), (12, 1));
    }

    #[tokio::test]
    async fn lookup_rejects_a_different_signature_scope_origin_or_history() {
        let game = extract("1. e4 e5 2. Nf3").unwrap();
        let review = ContextualReview::new(&game, "sf19", None);
        let budget = EvidenceBudget {
            mode: Mode::Time,
            value: 100,
            multipv: 1,
        };
        let base = review.entry(3, &raw(&game.fens[3]), Some("sf".into()), budget);

        let mut wrong_sig = base.clone();
        wrong_sig.evidence.engine_build = Some("other-build".into());
        let mut no_sig = base.clone();
        no_sig.evidence.engine_build = None;
        let mut wrong_scope = base.clone();
        wrong_scope.evidence.scope = EvidenceScope::FenOnly;
        let mut cached = base.clone();
        cached.evidence.origin = EvidenceOrigin::Cache;
        let mut terminal = base.clone();
        terminal.evidence.origin = EvidenceOrigin::Terminal;
        let mut wrong_history = base.clone();
        wrong_history.evidence.history_ply = Some(2);
        let mut no_history = base.clone();
        no_history.evidence.history_ply = None;
        let mut wrong_key = base.clone();
        wrong_key.key = review.keys[2].clone();
        let mut wrong_fen = base.clone();
        wrong_fen.raw.fen = game.fens[2].clone();

        for (label, bad) in [
            ("signature", wrong_sig),
            ("missing signature", no_sig),
            ("scope", wrong_scope),
            ("cache origin", cached),
            ("terminal origin", terminal),
            ("history", wrong_history),
            ("missing history", no_history),
            ("key", wrong_key),
            ("raw fen", wrong_fen),
        ] {
            let repo = ContextRepo::with_hits(vec![Some(bad)]);
            let results = review
                .lookup(&repo, &[3], Mode::Time, 100, 1)
                .await
                .unwrap();
            assert!(
                results[0].is_none(),
                "{label} must invalidate a contextual hit"
            );
        }

        let repo = ContextRepo::with_hits(vec![Some(base)]);
        let results = review
            .lookup(&repo, &[3], Mode::Time, 100, 1)
            .await
            .unwrap();
        assert!(
            results[0].is_some(),
            "an exact-context hit must be accepted"
        );
    }

    #[tokio::test]
    async fn lookup_does_not_reuse_evidence_from_a_different_history() {
        let a = extract("1. Nf3 Nc6 2. Nc3 Nf6").unwrap();
        let b = extract("1. Nc3 Nf6 2. Nf3 Nc6").unwrap();
        let ra = ContextualReview::new(&a, "sf19", None);
        let rb = ContextualReview::new(&b, "sf19", None);
        assert_eq!(a.fens[4], b.fens[4]);
        let budget = EvidenceBudget {
            mode: Mode::Depth,
            value: 12,
            multipv: 1,
        };
        let hit_from_a = ra.entry(4, &raw(&a.fens[4]), Some("sf".into()), budget);
        let repo = ContextRepo::with_hits(vec![Some(hit_from_a)]);

        let results = rb.lookup(&repo, &[4], Mode::Depth, 12, 1).await.unwrap();
        assert!(
            results[0].is_none(),
            "a hit keyed by another history must be a miss"
        );
        assert_eq!(repo.seen.lock().unwrap()[0].0, vec![rb.keys[4].clone()]);
    }

    #[tokio::test]
    async fn lookup_rejects_a_mismatched_result_count_and_propagates_errors() {
        let game = extract("1. e4").unwrap();
        let review = ContextualReview::new(&game, "sf19", None);

        let short = ContextRepo::with_hits(vec![]);
        let error = review
            .lookup(&short, &[1], Mode::Depth, 12, 1)
            .await
            .unwrap_err();
        assert_eq!(error.code, ReviewErrorCode::Cache);
        assert_eq!(error.operation, "cache.context.lookup");

        let mut failing = ContextRepo::with_hits(vec![None]);
        failing.fail = true;
        let error = review
            .lookup(&failing, &[1], Mode::Depth, 12, 1)
            .await
            .unwrap_err();
        assert_eq!(error.code, ReviewErrorCode::Cache);
        assert_eq!(error.operation, "cache.context.lookup");
    }
}
