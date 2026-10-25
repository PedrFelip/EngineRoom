mod chess_com;
mod http;
mod tasks;

use chess_com::{Cursor, ImportError};
pub use tasks::ImportTasks;
use tauri::{ipc::Channel, WebviewWindow};

pub fn state() -> Result<ImportTasks, ImportError> {
    Ok(ImportTasks::new(std::sync::Arc::new(
        http::ChessComHttp::new()?,
    )))
}

#[tauri::command]
pub fn chess_com_search_start(
    window: WebviewWindow,
    state: tauri::State<'_, ImportTasks>,
    request_id: String,
    username: String,
    cursor: Option<Cursor>,
    on_event: Channel<tasks::Event>,
) -> Result<(), ImportError> {
    state.start(
        window.label(),
        &request_id,
        username,
        cursor,
        move |event| {
            let _ = on_event.send(event);
        },
    )
}

#[tauri::command]
pub async fn chess_com_search_cancel(
    window: WebviewWindow,
    state: tauri::State<'_, ImportTasks>,
    request_id: String,
) -> Result<(), ImportError> {
    state.cancel(window.label(), &request_id).await
}

#[cfg(test)]
mod tests;
