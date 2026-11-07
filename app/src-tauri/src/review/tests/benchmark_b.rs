//! Explicit, exploratory paired comparison with a contextual deeper reference.
use super::*;
use crate::review::decision::{planner, runner, EngineSignature};
use crate::review::repository::SqliteRepository;
use serde_json::json;
use std::time::Instant;
use tauri::Manager;

/// Includes Stockfish, SQLite, engine setup and teardown; excludes UI and IPC.
/// One complete game and two runs per candidate are exploratory, not adoption evidence.
#[tokio::test]
#[ignore = "explicit slow paired Stockfish experiment"]
async fn benchmark_b_paired() {
    let corpus = include_str!("../../../../benchmarks/games.pgn");
    let pgn = corpus.split("\n\n[Event ").next().unwrap();
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_shell::init())
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    app.manage(db::DbState(Mutex::new(db::open_memory().unwrap())));
    let factory = Arc::new(SidecarFactory {
        app: app.handle().clone(),
        permit: Arc::new(Semaphore::new(1)),
    });
    let repo = Arc::new(SqliteRepository::new(app.handle().clone()));
    let (_closed, cancel) = cancel();
    let digest = factory.executable_digest().unwrap();
    let b_config = |screening_depth, verification_depth| planner::Config {
        generation: 1,
        engine: EngineSignature {
            executable_digest: digest.clone(),
            options: vec![
                ("Hash".into(), "16".into()),
                ("MultiPV".into(), "3".into()),
                ("Threads".into(), "1".into()),
            ],
        },
        screening_depth,
        multipv: 3,
        verification_depth,
        critical_cp: 100,
        max_attempts: 1,
    };
    let mut samples = Vec::new();
    // A-B-B-A reduces simple time-order bias; reference is run last, cold.
    for label in ["legacy", "b", "b", "legacy", "reference"] {
        {
            let state = app.state::<db::DbState>();
            Cache::new(&state.0.lock().unwrap()).clear().unwrap();
        }
        eprintln!("B paired experiment: {label}");
        let start = Instant::now();
        let result = if label == "legacy" {
            let mut pipeline = Pipeline::new(factory.clone(), repo.clone());
            let result = pipeline
                .review(
                    &config(pgn, AnalysisKind::Fast),
                    Some((2, 32)),
                    &cancel,
                    &mut |_| {},
                )
                .await
                .unwrap();
            pipeline.close().await;
            json!(result)
        } else {
            let (screen, verify) = if label == "reference" {
                (14, 18)
            } else {
                (8, 12)
            };
            let mut plan = planner::Planner::new(pgn, b_config(screen, verify)).unwrap();
            let snapshot = runner::run_workers(
                factory.as_ref(),
                &mut plan,
                Duration::from_secs(120),
                &cancel,
                2,
            )
            .await
            .unwrap();
            json!(snapshot)
        };
        samples.push(
            json!({"kind": label, "elapsedMs": start.elapsed().as_millis(), "result": result}),
        );
    }
    let report = json!({
        "schema": 1, "corpus": "benchmarks/games.pgn:first game, Larsen-Spassky 1970",
        "pgn": pgn, "engineSha256": digest, "resources": {"threads": 2, "hashMiB": 32},
        "cache": "cold", "order": ["legacy", "b", "b", "legacy", "reference"],
        "legacy": {"profile": "fast", "multipv": 3},
        "b": {"screeningDepth": 8, "verificationDepth": 12, "multipv": 3},
        "reference": {"screeningDepth": 14, "verificationDepth": 18, "multipv": 3},
        "excludes": ["frontend IPC", "rendering"], "samples": samples
    });
    let path =
        std::env::var("BENCH_REPORT_PATH").expect("BENCH_REPORT_PATH must name a new output file");
    use std::io::Write;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    output
        .write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .unwrap();
}
