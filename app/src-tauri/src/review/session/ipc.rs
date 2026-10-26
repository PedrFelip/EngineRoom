//! Tauri adapters and production dependency wiring for review sessions.
use super::{
    remove,
    task::run_session,
    update,
    validation::{validate, validate_live},
    Job, ReviewSessions,
};
use crate::review::{
    engine::{self, Cancellation, EngineFactory, SidecarFactory},
    pipeline::Pipeline,
    repository::SqliteRepository,
    types::{
        Envelope, IpcResult, LiveRequest, LiveSettings, Result, ReviewConfig, ReviewError,
        ReviewErrorCode, ReviewErrorPayload,
    },
};
use std::sync::Arc;
use tauri::{ipc::Channel, WebviewWindow};

#[tauri::command]
pub fn review_session_open(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    config: ReviewConfig,
    on_event: Channel<Envelope>,
) -> IpcResult<()> {
    (|| -> Result<_> {
        validate(&config)?;
        let (session, finished, done) = state.register(&session_id, window.label())?;
        let sessions = state.sessions.clone();
        let factory = Arc::new(SidecarFactory {
            app: app.clone(),
            permit: state.permit.clone(),
        });
        tauri::async_runtime::spawn(async move {
            run_session(
                &session_id,
                &config,
                &session,
                finished,
                done,
                Pipeline::new(factory, Arc::new(SqliteRepository::new(app))),
                &mut |envelope| on_event.send(envelope).is_ok(),
            )
            .await;
            remove(&sessions, &session_id, &session);
        });
        Ok(())
    })()
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}
#[tauri::command]
pub fn review_session_analyze_position(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    request_id: u64,
    request: LiveRequest,
    settings: LiveSettings,
) -> IpcResult<()> {
    (|| -> Result<_> {
        validate_live(&settings)?;
        let session = state.get(&session_id, window.label())?;
        update(
            &session,
            request_id,
            Some(Job {
                id: request_id,
                request,
                settings,
            }),
        )
    })()
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}
#[tauri::command]
pub async fn review_session_cancel_live(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    request_id: u64,
) -> IpcResult<()> {
    let session = state
        .get(&session_id, window.label())
        .map_err(ReviewErrorPayload::from)?;
    session
        .cancel_live(request_id)
        .await
        .map_err(ReviewErrorPayload::from)
}
#[tauri::command]
pub async fn review_session_close(
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
) -> IpcResult<()> {
    async {
        let s = match state.get(&session_id, window.label()) {
            Ok(s) => s,
            Err(e) if e.code == ReviewErrorCode::SessionClosed => return Ok(()),
            Err(e) => return Err(e),
        };
        s.close().await;
        Ok(())
    }
    .await
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}
#[derive(Clone, serde::Serialize)]
pub struct ProbeResult {
    pub ok: bool,
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[tauri::command]
pub fn engine_probe(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, ReviewSessions>,
    session_id: String,
    on_event: Channel<ProbeResult>,
    timeout_ms: Option<u64>,
) -> IpcResult<()> {
    (|| -> Result<_> {
        let timeout = timeout_ms.unwrap_or(8000).clamp(1, 30000);
        let (session, _, done) = state.register(&session_id, window.label())?;
        let sessions = state.sessions.clone();
        let factory = SidecarFactory {
            app,
            permit: state.permit.clone(),
        };
        tauri::async_runtime::spawn(async move {
            let cancel = Cancellation {
                closed: session.close.subscribe(),
                revision: None,
            };
            let mut owned = None;
            let result = async {
                let port = factory.acquire(&cancel).await?;
                let port = owned.insert(port);
                let lines = engine::ask(port.as_mut(), "uci", "uciok", timeout, &cancel).await?;
                Ok::<_, ReviewError>(
                    lines
                        .iter()
                        .find_map(|l| l.strip_prefix("id name ").map(str::to_owned)),
                )
            }
            .await;
            if let Some(mut port) = owned {
                port.shutdown().await;
            }
            let payload = match result {
                Ok(name) => ProbeResult {
                    ok: true,
                    name,
                    error: None,
                },
                Err(e) => ProbeResult {
                    ok: false,
                    name: None,
                    error: Some(e.message),
                },
            };
            let _ = on_event.send(payload);
            done.send_replace(true);
            remove(&sessions, &session_id, &session);
        });
        Ok(())
    })()
    .map_err(|error: ReviewError| ReviewErrorPayload::from(error))
}
