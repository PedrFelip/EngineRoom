//! Context-preserving UCI driver for the experimental decision pipeline.
use super::{satisfies, Bound, Evidence, Line, Request, Score};
use crate::review::{
    core,
    engine::{Cancellation, EnginePort},
    types::{Result, ReviewError, ReviewErrorCode},
};
use shakmaty::Position;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

fn error(code: ReviewErrorCode, message: impl ToString) -> ReviewError {
    ReviewError::new(code, "decision.driver", message)
}

/// Parse only UCI analysis records. Scores remain relative to the side to move.
fn parse_info(text: &str, execution_id: u64) -> Option<(usize, Line)> {
    let words: Vec<_> = text.split_whitespace().collect();
    if words.first()? != &"info" {
        return None;
    }
    let mut depth = None;
    let mut slot = 1;
    let mut score = None;
    let mut bound = Bound::Exact;
    let mut pv = None;
    let mut i = 1;
    while i < words.len() {
        match words[i] {
            "string" => break,
            "depth" => {
                i += 1;
                depth = words.get(i)?.parse().ok();
            }
            "multipv" => {
                i += 1;
                slot = words.get(i)?.parse().ok()?;
            }
            "score" => {
                let n: i32 = words.get(i + 2)?.parse().ok()?;
                n.checked_neg()?;
                score = match *words.get(i + 1)? {
                    "cp" => Some(Score::Cp(n)),
                    "mate" => Some(Score::Mate(n)),
                    _ => None,
                };
                i += 2;
            }
            "lowerbound" => bound = Bound::Lower,
            "upperbound" => bound = Bound::Upper,
            "pv" => {
                pv = Some(words[i + 1..].iter().map(|s| (*s).to_owned()).collect());
                break;
            }
            _ => {}
        }
        i += 1;
    }
    Some((
        slot,
        Line {
            depth: depth?,
            execution_id,
            score: score?,
            bound,
            pv: pv?,
        },
    ))
}

/// A single mutable borrow serializes the reader and searches on this process.
/// The owner must always await `EnginePort::shutdown`, even on errors.
pub async fn search(
    port: &mut dyn EnginePort,
    request: &Request,
    execution_id: u64,
    timeout: Duration,
    cancel: &Cancellation,
) -> Result<(Evidence, Duration)> {
    cancel.check()?;
    super::position_status(&request.context)?;
    if request.depth == 0
        || request.multipv == 0
        || request.engine.options.windows(2).any(|w| w[0].0 >= w[1].0)
    {
        return Err(error(
            ReviewErrorCode::EngineProtocol,
            "invalid request or noncanonical options",
        ));
    }
    let position = core::position(&request.context.fen)?;
    let slots = request.multipv.min(position.legal_moves().len());
    if slots == 0 {
        return Err(error(
            ReviewErrorCode::MissingEvaluation,
            "terminal position",
        ));
    }
    let multipv = request
        .engine
        .options
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("MultiPV"));
    if multipv.and_then(|(_, value)| value.parse::<usize>().ok()) != Some(request.multipv) {
        return Err(error(
            ReviewErrorCode::EngineProtocol,
            "signature must include requested MultiPV",
        ));
    }
    for (name, value) in &request.engine.options {
        if name.contains(['\n', '\r']) || value.contains(['\n', '\r']) {
            return Err(error(
                ReviewErrorCode::EngineProtocol,
                "invalid engine option",
            ));
        }
        port.send(&format!("setoption name {name} value {value}"))?;
    }
    port.send("isready")?;
    let start = Instant::now();
    let work = async {
        loop {
            if port.next().await?.trim() == "readyok" {
                break;
            }
        }
        let mut command = format!("position fen {}", request.context.root_fen);
        if !request.context.prefix.is_empty() {
            command.push_str(" moves ");
            command.push_str(&request.context.prefix.join(" "));
        }
        port.send(&command)?;
        port.send(&format!("go depth {}", request.depth))?;
        let mut depths: BTreeMap<u32, BTreeMap<usize, Line>> = BTreeMap::new();
        let mut best: Option<Evidence> = None;
        loop {
            let text = port.next().await?;
            if text.trim().starts_with("bestmove ") {
                break;
            }
            if let Some((slot, line)) = parse_info(&text, execution_id) {
                if slot == 0 || slot > slots {
                    continue;
                }
                let depth = line.depth;
                let group = depths.entry(depth).or_default();
                // Slot 1 starts a new ordered MultiPV cycle at this depth.
                if slot == 1 {
                    group.clear();
                }
                if slot != group.len() + 1 {
                    group.clear();
                    continue;
                }
                group.insert(slot, line);
                if (1..=slots).all(|i| group.contains_key(&i)) {
                    let evidence = Evidence {
                        id: execution_id,
                        execution_id,
                        context: request.context.clone(),
                        engine: request.engine.clone(),
                        unrestricted: true,
                        depth,
                        lines: (1..=slots).map(|i| group[&i].clone()).collect(),
                    };
                    if satisfies(&evidence, request)
                        && best.as_ref().is_none_or(|b| depth >= b.depth)
                    {
                        best = Some(evidence);
                    }
                }
                // Older incomplete cycles cannot contribute once a newer depth arrives.
                // Retain only the current depth and the last complete evidence above.
                depths.retain(|d, _| *d >= depth);
                while depths.len() > 2 {
                    depths.pop_first();
                }
            }
        }
        best.ok_or_else(|| {
            error(
                ReviewErrorCode::MissingEvaluation,
                "no complete legal exact MultiPV set",
            )
        })
    };
    let result = tokio::select! {
        _ = cancel.cancelled() => Err(ReviewError::cancelled()),
        result = tokio::time::timeout(timeout, work) => result.unwrap_or_else(|_| Err(error(ReviewErrorCode::EngineTimeout, "search timed out"))),
    }?;
    Ok((result, start.elapsed()))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        lines: std::collections::VecDeque<String>,
        commands: Vec<String>,
    }
    impl EnginePort for Fake {
        fn send(&mut self, command: &str) -> Result<()> {
            self.commands.push(command.into());
            Ok(())
        }
        fn next(&mut self) -> crate::review::engine::Task<'_, Result<String>> {
            Box::pin(async move {
                self.lines
                    .pop_front()
                    .ok_or_else(|| error(ReviewErrorCode::EngineExited, "EOF"))
            })
        }
        fn shutdown(&mut self) -> crate::review::engine::Task<'_, ()> {
            Box::pin(async {})
        }
    }
    #[tokio::test]
    async fn coherent_complete_set_survives_partial_and_invalid_updates() {
        let context = super::super::decisions("1. e4 *").unwrap()[0]
            .before
            .clone();
        let request = Request {
            context,
            engine: super::super::EngineSignature {
                executable_digest: "a".repeat(64),
                options: vec![("MultiPV".into(), "2".into())],
            },
            depth: 8,
            multipv: 2,
        };
        let mut fake = Fake {
            commands: vec![],
            lines: [
                "readyok",
                "info depth 8 multipv 1 score cp 20 pv e2e4",
                "info depth 7 multipv 2 score cp 10 pv d2d4",
                "info depth 8 multipv 2 score cp 10 pv d2d4",
                "info depth 8 multipv 1 score cp 99 pv g1f3",
                "info depth 9 multipv 2 score cp 90 pv d2d4",
                "info depth 9 multipv 1 score cp 50 pv g1f3",
                "info depth 9 multipv 2 score cp 40 lowerbound pv d2d4",
                "bestmove e2e4",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        };
        let (_tx, rx) = tokio::sync::watch::channel(false);
        let cancel = Cancellation {
            closed: rx,
            revision: None,
        };
        let (e, _) = search(&mut fake, &request, 9, Duration::from_secs(1), &cancel)
            .await
            .unwrap();
        assert_eq!(e.depth, 8);
        assert_eq!(e.lines[0].pv[0], "e2e4");
        assert!(fake.commands.iter().any(|c| c.starts_with("position fen ")));
        assert_eq!(
            fake.commands
                .iter()
                .filter(|c| c.starts_with("go "))
                .count(),
            1
        );
    }
    #[test]
    fn parser_preserves_bounds_and_mate() {
        for kind in ["cp", "mate"] {
            assert!(
                parse_info(&format!("info depth 8 score {kind} -2147483648 pv e2e4"), 1).is_none()
            );
        }
        let (_, line) =
            parse_info("info depth 9 multipv 2 score mate -3 upperbound pv e2e4", 4).unwrap();
        assert_eq!(line.score, Score::Mate(-3));
        assert_eq!(line.bound, Bound::Upper);
        assert!(parse_info("info string depth 4 score cp 2 pv e2e4", 1).is_none());
    }
}
