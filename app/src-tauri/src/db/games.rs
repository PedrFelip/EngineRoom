//! Store de partidas revisadas: CRUD sobre a tabela `games`.

use crate::db::{mode::Mode, DbState};
use rusqlite::Connection;

/// Partida revisada a gravar (payload do frontend, sem id/created_at).
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewGame {
    pub pgn: String,
    pub white: String,
    pub black: String,
    pub result: String,
    pub plies: u32,
    pub engine_tier: String,
    pub mode: Mode,
    pub analysis_kind: String,
    pub depth: u32,
    pub multipv: u32,
    pub accuracy_white: f64,
    pub accuracy_black: f64,
    pub review_json: String,
}

/// Linha da lista da home: metadados sem o peso do pgn/review_json.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
    pub id: i64,
    pub white: String,
    pub black: String,
    pub result: String,
    pub plies: u32,
    pub engine_tier: String,
    pub mode: Mode,
    pub analysis_kind: String,
    pub depth: u32,
    pub multipv: u32,
    pub accuracy_white: f64,
    pub accuracy_black: f64,
    pub created_at: String,
}

/// Partida completa, para reabertura instantânea da revisão.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredGame {
    #[serde(flatten)]
    pub summary: GameSummary,
    pub pgn: String,
    pub review_json: String,
}

/// Vista sobre uma [`Connection`] para operações sobre o store de partidas.
pub struct Store<'a> {
    conn: &'a Connection,
}

const SUMMARY_COLS: &str = "id, white, black, result, plies, engine_tier, mode, analysis_kind, depth, multipv,
        accuracy_white, accuracy_black, created_at";

impl<'a> Store<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn save(&self, game: &NewGame) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO games
                (pgn, white, black, result, plies, engine_tier, mode, analysis_kind,
                 depth, multipv, accuracy_white, accuracy_black, review_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                (
                    &game.pgn,
                    &game.white,
                    &game.black,
                    &game.result,
                    game.plies,
                    &game.engine_tier,
                    game.mode,
                    &game.analysis_kind,
                    game.depth,
                    game.multipv,
                    game.accuracy_white,
                    game.accuracy_black,
                    &game.review_json,
                ),
            )
            .map_err(|e| e.to_string())?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list(&self) -> Result<Vec<GameSummary>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {SUMMARY_COLS} FROM games ORDER BY created_at DESC, id DESC"
            ))
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(()).map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            out.push(summary_from_row(row)?);
        }
        Ok(out)
    }

    pub fn get(&self, id: i64) -> Result<Option<StoredGame>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {SUMMARY_COLS}, pgn, review_json FROM games WHERE id = ?1"
            ))
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query((id,)).map_err(|e| e.to_string())?;
        match rows.next().map_err(|e| e.to_string())? {
            Some(row) => Ok(Some(stored_from_row(row)?)),
            None => Ok(None),
        }
    }

    pub fn remove(&self, id: i64) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM games WHERE id = ?1", (id,))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM games", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Mapeia as colunas (ordem de `SUMMARY_COLS`) para [`GameSummary`].
fn summary_from_row(row: &rusqlite::Row<'_>) -> Result<GameSummary, String> {
    Ok(GameSummary {
        id: row.get(0).map_err(|e| e.to_string())?,
        white: row.get(1).map_err(|e| e.to_string())?,
        black: row.get(2).map_err(|e| e.to_string())?,
        result: row.get(3).map_err(|e| e.to_string())?,
        plies: row.get(4).map_err(|e| e.to_string())?,
        engine_tier: row.get(5).map_err(|e| e.to_string())?,
        mode: row.get(6).map_err(|e| e.to_string())?,
        analysis_kind: row.get(7).map_err(|e| e.to_string())?,
        depth: row.get(8).map_err(|e| e.to_string())?,
        multipv: row.get(9).map_err(|e| e.to_string())?,
        accuracy_white: row.get(10).map_err(|e| e.to_string())?,
        accuracy_black: row.get(11).map_err(|e| e.to_string())?,
        created_at: row.get(12).map_err(|e| e.to_string())?,
    })
}

fn stored_from_row(row: &rusqlite::Row<'_>) -> Result<StoredGame, String> {
    Ok(StoredGame {
        summary: summary_from_row(row)?,
        pgn: row.get(13).map_err(|e| e.to_string())?,
        review_json: row.get(14).map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
pub fn games_save(state: tauri::State<'_, DbState>, game: NewGame) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Store::new(&conn).save(&game)
}

#[tauri::command]
pub fn games_list(state: tauri::State<'_, DbState>) -> Result<Vec<GameSummary>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Store::new(&conn).list()
}

#[tauri::command]
pub fn games_get(state: tauri::State<'_, DbState>, id: i64) -> Result<Option<StoredGame>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Store::new(&conn).get(id)
}

#[tauri::command]
pub fn games_delete(state: tauri::State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Store::new(&conn).remove(id)
}

#[tauri::command]
pub fn games_clear(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Store::new(&conn).clear()
}

#[cfg(test)]
#[path = "games/tests.rs"]
mod tests;
