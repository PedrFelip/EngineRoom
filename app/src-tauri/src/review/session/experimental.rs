use crate::review::{
    decision::{self, planner, policy::Value, runner, Score},
    engine::{Cancellation, EngineFactory},
    repository::Repository,
    types::{Event, Result},
};
use serde_json::{json, Value as Json};
use std::time::Duration;
fn value(v: &Value) -> Json {
    match v {
        Value::Engine(Score::Cp(n)) => json!({"kind":"cp","value":n}),
        Value::Engine(Score::Mate(n)) => json!({"kind":"mate","moves":n}),
        Value::Terminal(s) => json!({"kind":"terminal","outcome":format!("{s:?}")}),
    }
}
fn project(s: &planner::Snapshot, fens: &[String]) -> Json {
    let positions: Vec<_> = fens
        .iter()
        .enumerate()
        .map(|(ply, fen)| json!({"ply":ply,"fen":fen}))
        .collect();
    let decisions: Vec<_> = s.decisions.iter().map(|d| {
  let c = s.revisions.iter().rev().find(|c| c.ply == d.ply).or_else(|| s.screening.iter().find(|c| c.ply == d.ply));
  let reason = s.unresolved.iter().find(|p| p.ply == d.ply).map(|p| p.reason.as_str());
  let evidence = c.and_then(|c| c.best_evidence).and_then(|id| s.evidence.iter().find(|e| e.id == id));
  let lines: Vec<_> = evidence.into_iter().flat_map(|e| &e.lines).map(|l| json!({"pv":l.pv,"depth":l.depth,"value":value(&Value::Engine(l.score.clone()))})).collect();
  let verified = s.verified_plies.contains(&d.ply);
  json!({"ply":d.ply,"version":c.map_or(1,|c|c.version),"status":if reason.is_some() || (s.complete && c.is_none()) {"unresolved"} else if verified {"verified"} else {"preliminary"},"best":c.map(|c|value(&c.best)),"played":c.map(|c|value(&c.played)),"bestUci":lines.first().and_then(|l|l["pv"].get(0)).cloned(),"lines":lines,"reason":reason.or(if s.complete && c.is_none(){Some("screening evidence unavailable")}else{None})})
 }).collect();
    json!({"version":1,"pipeline":"experimental-b","persistence":"temporary","status":if s.complete && s.unresolved.is_empty(){"protocolCompleted"}else{"unresolved"},"positions":positions,"decisions":decisions})
}
pub(super) async fn run(
    config: &crate::review::types::ReviewConfig,
    factory: &dyn EngineFactory,
    repository: &dyn Repository,
    cancel: &Cancellation,
    publish: &mut (dyn FnMut(Event) + Send),
) -> Result<(Json, planner::Snapshot)> {
    let game = crate::review::core::extract(&config.pgn)?;
    publish(Event::BProgress {
        stage: "coverage".into(),
        completed: 0,
        total: game.moves.len(),
    });
    cancel.check()?;
    let digest = factory.executable_digest()?;
    let mut plan = planner::Planner::new(
        &config.pgn,
        planner::Config {
            generation: 1,
            engine: decision::EngineSignature {
                executable_digest: digest,
                options: vec![
                    ("Hash".into(), "16".into()),
                    ("MultiPV".into(), config.lines.to_string()),
                    ("Threads".into(), "1".into()),
                ],
            },
            screening_depth: 8,
            multipv: config.lines as usize,
            verification_depth: 12,
            critical_cp: 100,
            max_attempts: 1,
        },
    )?;
    let (updates, mut snapshots) = tokio::sync::watch::channel(plan.snapshot());
    let execution = runner::run_workers_cached_progress(
        factory,
        &mut plan,
        Duration::from_secs(120),
        cancel,
        2,
        Some(repository),
        Some(&updates),
    );
    tokio::pin!(execution);
    let s = loop {
        tokio::select! {
            result = &mut execution => break result?,
            changed = snapshots.changed() => {
                if changed.is_ok() {
                    let snapshot = snapshots.borrow_and_update();
                    publish(Event::BProgress {
                        stage: "coverage".into(),
                        completed: snapshot.screening.len(),
                        total: game.moves.len(),
                    });
                    publish(Event::BUpdated { result: project(&snapshot, &game.fens) });
                }
            }
        }
    };
    publish(Event::BProgress {
        stage: "coverage".into(),
        completed: game.moves.len(),
        total: game.moves.len(),
    });
    Ok((project(&s, &game.fens), s))
}
