use crate::db::mode::Mode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewErrorCode {
    Cancelled,
    Cache,
    Persistence,
    Session,
    SessionClosed,
    SessionExists,
    InvalidPayload,
    InvalidPgn,
    EngineSpawn,
    EngineExited,
    EngineCommand,
    EngineTimeout,
    EngineProtocol,
    MissingEvaluation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewError {
    pub code: ReviewErrorCode,
    pub operation: String,
    pub message: String,
}
impl ReviewError {
    pub fn new(code: ReviewErrorCode, operation: &str, message: impl ToString) -> Self {
        Self {
            code,
            operation: operation.into(),
            message: message.to_string(),
        }
    }
    pub fn cancelled() -> Self {
        Self::new(ReviewErrorCode::Cancelled, "session", "Análise cancelada.")
    }
}
pub type Result<T> = std::result::Result<T, ReviewError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Opening,
    Middlegame,
    Endgame,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Classification {
    Livro,
    Melhor,
    Excelente,
    Bom,
    Imprecisao,
    Erro,
    Blunder,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawLine {
    /// Last completed depth changed the PV root or win chance by at least 2pp.
    #[serde(default)]
    pub unstable: bool,
    pub multipv: u32,
    pub cp: i32,
    pub pv: Vec<String>,
    #[serde(default)]
    pub san: Option<String>,
    #[serde(default)]
    pub depth: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawPosition {
    pub fen: String,
    pub cp: i32,
    pub depth: u32,
    pub pv: Vec<String>,
    pub lines: Vec<RawLine>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayedMove {
    pub ply: usize,
    pub color: String,
    pub san: String,
    pub uci: String,
    pub fen_before: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Eco {
    pub code: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveAnalysis {
    #[serde(flatten)]
    pub played: PlayedMove,
    pub classification: Classification,
    pub win_pct_before: f64,
    pub win_pct_after: f64,
    pub win_pct_loss: f64,
    pub cp_loss: f64,
    pub best_uci: Option<String>,
    pub is_book: bool,
    pub eco: Option<Eco>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PvLine {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    pub multipv: u32,
    pub san: Option<String>,
    pub cp: i32,
    pub win_pct: f64,
    pub pv: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Search {
    pub purpose: String,
    pub movetime_ms: u32,
    pub multipv: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionAnalysis {
    pub ply: usize,
    pub fen: String,
    pub phase: Phase,
    pub depth: u32,
    pub cp: i32,
    pub win_pct: f64,
    pub pv: Vec<String>,
    pub lines: Vec<PvLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<Search>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triage_lines: Option<Vec<PvLine>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accuracy {
    pub white: f64,
    pub black: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseAccuracy {
    pub opening: Accuracy,
    pub middlegame: Accuracy,
    pub endgame: Accuracy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewResult {
    pub positions: Vec<PositionAnalysis>,
    pub moves: Vec<MoveAnalysis>,
    pub accuracy_model: String,
    pub accuracy: Accuracy,
    pub accuracy_by_phase: PhaseAccuracy,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineTier {
    pub id: String,
    pub depth: u32,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub hint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PgnMeta {
    pub white: String,
    pub black: String,
    pub white_elo: Option<String>,
    pub black_elo: Option<String>,
    pub result: String,
    pub event: Option<String>,
    pub plies: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AnalysisKind {
    #[default]
    #[serde(rename = "manual")]
    Manual,
    #[serde(rename = "auto-fast")]
    Fast,
    #[serde(rename = "auto-deep")]
    Deep,
}
impl AnalysisKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Fast => "auto-fast",
            Self::Deep => "auto-deep",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewConfig {
    pub pgn: String,
    pub meta: PgnMeta,
    pub engine: EngineTier,
    pub mode: Mode,
    #[serde(default)]
    pub analysis_kind: AnalysisKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movetime_ms: Option<u32>,
    pub lines: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_result: Option<ReviewResult>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveRequest {
    pub fen: String,
    #[serde(default)]
    pub variation_node_id: Option<String>,
    #[serde(default)]
    pub source_fen: Option<String>,
    #[serde(default)]
    pub source_analysis: Option<PositionAnalysis>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveSettings {
    pub search_seconds: u32,
    pub lines: u32,
    pub threads_auto: bool,
    pub threads: u32,
    pub memory_mb: u32,
    pub move_feedback_enabled: bool,
    #[serde(default)]
    pub fast_pass: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub stage: String,
    pub completed: usize,
    pub total: usize,
    pub current_ply: usize,
    pub phase: Option<Phase>,
    pub cached_positions: usize,
    pub engine_positions: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_budget_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update: Option<WinPctUpdate>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WinPctUpdate {
    pub index: usize,
    pub win_pct: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Progress {
        progress: Progress,
    },
    Completed {
        result: ReviewResult,
    },
    LiveStarted {
        fen: String,
    },
    LiveCompleted {
        fen: String,
        analysis: PositionAnalysis,
    },
    Classification {
        #[serde(rename = "nodeId")]
        node_id: String,
        classification: Classification,
    },
    Error {
        error: ReviewError,
        #[serde(skip_serializing_if = "Option::is_none")]
        fen: Option<String>,
    },
    Warning {
        error: ReviewError,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub session_id: String,
    pub sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<u64>,
    #[serde(flatten)]
    pub event: Event,
}

#[cfg(test)]
mod error_tests {
    use super::*;

    #[test]
    fn error_codes_preserve_ipc_strings() {
        use ReviewErrorCode::*;
        let cases = [
            (Cancelled, "cancelled"),
            (Cache, "cache"),
            (Persistence, "persistence"),
            (Session, "session"),
            (SessionClosed, "sessionClosed"),
            (SessionExists, "sessionExists"),
            (InvalidPayload, "invalidPayload"),
            (InvalidPgn, "invalidPgn"),
            (EngineSpawn, "engineSpawn"),
            (EngineExited, "engineExited"),
            (EngineCommand, "engineCommand"),
            (EngineTimeout, "engineTimeout"),
            (EngineProtocol, "engineProtocol"),
            (MissingEvaluation, "missingEvaluation"),
        ];
        for (code, wire_code) in cases {
            let payload = serde_json::json!({
                "code": wire_code,
                "operation": "test",
                "message": "failure",
            });
            let error = ReviewError::new(code, "test", "failure");
            assert_eq!(serde_json::to_value(&error).unwrap(), payload);
            let decoded: ReviewError = serde_json::from_value(payload).unwrap();
            assert_eq!(decoded.code, code);
        }
    }

    #[test]
    fn unknown_error_codes_are_rejected() {
        assert!(serde_json::from_value::<ReviewError>(serde_json::json!({
            "code": "engineTimout",
            "operation": "test",
            "message": "failure",
        }))
        .is_err());
    }
}
