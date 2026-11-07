use app_lib::review::{
    decision::{
        planner::{Config, Planner, Snapshot},
        runner::{self, process::Factory},
        EngineSignature,
    },
    engine::Cancellation,
    types::{ReviewError, ReviewErrorCode, ReviewErrorPayload},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    version: u32,
    screening_depth: u32,
    verification_depth: u32,
    critical_cp: i32,
    max_attempts: u32,
    multipv: usize,
    threads: u32,
    hash_mb: u32,
    timeout_ms: u64,
    cancel_after_ms: Option<u64>,
}
impl Manifest {
    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.version != 1
            || self.screening_depth == 0
            || self.verification_depth <= self.screening_depth
            || self.critical_cp <= 0
            || self.max_attempts == 0
            || self.multipv == 0
            || self.multipv > 256
            || self.threads == 0
            || self.threads > 1024
            || self.hash_mb == 0
            || self.hash_mb > 33554432
            || self.timeout_ms == 0
        {
            return Err("invalid manifest version or parameter bounds".into());
        }
        Ok(())
    }
}
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    ProtocolCompleted,
    Unresolved,
    Cancelled,
    Failed,
}
#[derive(Serialize)]
struct Report {
    version: u32,
    pgn: String,
    executable: PathBuf,
    manifest: Manifest,
    signature: EngineSignature,
    status: Status,
    error: Option<ReviewErrorPayload>,
    snapshot: Snapshot,
}
fn status(result: &Result<Snapshot, ReviewError>, snapshot: &Snapshot) -> Status {
    match result {
        Err(e) if e.code == ReviewErrorCode::Cancelled => Status::Cancelled,
        Err(_) => Status::Failed,
        Ok(_) if !snapshot.complete || !snapshot.unresolved.is_empty() => Status::Unresolved,
        Ok(_) => Status::ProtocolCompleted,
    }
}
fn executable_digest(path: &Path) -> Result<(PathBuf, String), Box<dyn std::error::Error>> {
    let path = std::fs::canonicalize(path)?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err("engine must be a regular executable file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err("engine is not executable".into());
        }
    }
    let bytes = std::fs::read(&path)?;
    #[cfg(target_os = "linux")]
    let native = bytes.starts_with(b"\x7fELF");
    #[cfg(target_os = "windows")]
    let native = bytes.starts_with(b"MZ");
    #[cfg(target_os = "macos")]
    let native = bytes.get(..4).is_some_and(|m| {
        matches!(
            m,
            [0xfe, 0xed, 0xfa, 0xce]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xcf, 0xfa, 0xed, 0xfe]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
        )
    });
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    let native = false;
    if !native {
        return Err(
            "engine must be a native executable (scripts and unsupported formats are rejected)"
                .into(),
        );
    }
    Ok((path, format!("{:x}", Sha256::digest(&bytes))))
}
fn main() {
    if let Err(e) = execute() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn execute() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 5 {
        return Err("usage: analysis_b PGN STOCKFISH MANIFEST_JSON OUTPUT_JSON".into());
    }
    // create_new refuses existing files, including symlink and hard-link aliases of inputs.
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[4])?;
    let pgn = std::fs::read_to_string(&args[1])?;
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&args[3])?)?;
    manifest.validate()?;
    let (executable, digest) = executable_digest(Path::new(&args[2]))?;
    let signature = EngineSignature {
        executable_digest: digest,
        options: vec![
            ("Hash".into(), manifest.hash_mb.to_string()),
            ("MultiPV".into(), manifest.multipv.to_string()),
            ("Threads".into(), manifest.threads.to_string()),
        ],
    };
    let config = Config {
        generation: 1,
        engine: signature.clone(),
        screening_depth: manifest.screening_depth,
        multipv: manifest.multipv,
        verification_depth: manifest.verification_depth,
        critical_cp: manifest.critical_cp,
        max_attempts: manifest.max_attempts,
    };
    let mut plan = Planner::new(&pgn, config)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let (tx, rx) = tokio::sync::watch::channel(false);
        if let Some(ms) = manifest.cancel_after_ms {
            let tx = tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(ms)).await;
                let _ = tx.send(true);
            });
        }
        let cancel = Cancellation {
            closed: rx,
            revision: None,
        };
        let timeout = Duration::from_millis(manifest.timeout_ms);
        let factory = Factory {
            executable: executable.clone(),
            timeout,
        };
        runner::run(&factory, &mut plan, timeout, &cancel).await
    });
    let snapshot = plan.snapshot();
    let report = Report {
        version: 2,
        pgn,
        executable,
        manifest,
        signature,
        status: status(&result, &snapshot),
        error: result.as_ref().err().map(ReviewErrorPayload::from),
        snapshot,
    };
    output.write_all(&serde_json::to_vec_pretty(&report)?)?;
    result.map(|_| ()).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        serde_json::from_str(include_str!("fixtures/analysis-b-manifest.json")).unwrap()
    }

    fn snapshot() -> Snapshot {
        let m = manifest();
        Planner::new(
            "1. e4 *",
            Config {
                generation: 1,
                engine: EngineSignature {
                    executable_digest: "a".repeat(64),
                    options: vec![],
                },
                screening_depth: m.screening_depth,
                verification_depth: m.verification_depth,
                critical_cp: m.critical_cp,
                max_attempts: m.max_attempts,
                multipv: m.multipv,
            },
        )
        .unwrap()
        .snapshot()
    }

    #[test]
    fn output_cannot_truncate_an_existing_input() {
        let path = std::env::temp_dir().join(format!("analysis-b-output-{}", std::process::id()));
        std::fs::write(&path, b"original PGN").unwrap();
        let result = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original PGN");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn manifest_rejects_invalid_values_and_unknown_fields() {
        let mut m = manifest();
        assert!(m.validate().is_ok());
        m.verification_depth = m.screening_depth;
        assert!(m.validate().is_err());
        assert!(serde_json::from_str::<Manifest>(r#"{"version":1,"unexpected":true}"#).is_err());
    }

    #[test]
    fn cancellation_and_failure_remain_distinct() {
        let s = snapshot();
        assert_eq!(
            status(&Err(ReviewError::cancelled()), &s),
            Status::Cancelled
        );
        assert_eq!(
            status(
                &Err(ReviewError::new(
                    ReviewErrorCode::EngineTimeout,
                    "test",
                    "timeout"
                )),
                &s
            ),
            Status::Failed
        );
        assert_eq!(status(&Ok(s.clone()), &s), Status::Unresolved);
    }

    #[test]
    fn scripts_are_rejected_and_native_digest_matches_bytes() {
        let script = std::env::temp_dir().join(format!("analysis-b-script-{}", std::process::id()));
        std::fs::write(&script, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert!(executable_digest(&script).is_err());
        std::fs::remove_file(script).unwrap();
        let binary = std::env::current_exe().unwrap();
        let (_, digest) = executable_digest(&binary).unwrap();
        assert_eq!(
            digest,
            format!("{:x}", Sha256::digest(std::fs::read(binary).unwrap()))
        );
    }
}
