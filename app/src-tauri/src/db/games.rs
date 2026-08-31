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
mod tests {
    use super::*;
    use crate::db::open_memory;

    fn partida_exemplo() -> NewGame {
        NewGame {
            pgn: "1. e4 e5".to_string(),
            white: "Brancas".to_string(),
            black: "Pretas".to_string(),
            result: "1-0".to_string(),
            plies: 2,
            engine_tier: "balanced".to_string(),
            mode: Mode::Depth,
            analysis_kind: "manual".to_string(),
            depth: 20,
            multipv: 1,
            accuracy_white: 98.5,
            accuracy_black: 91.0,
            review_json: r#"{"positions":[],"moves":[]}"#.to_string(),
        }
    }

    #[test]
    fn partida_salva_em_modo_time_preserva_mode_e_movetime() {
        let conn = open_memory().unwrap();
        let mut game = partida_exemplo();
        game.engine_tier = "time".to_string();
        game.mode = Mode::Time;
        game.depth = 5000; // movetimeMs
        game.multipv = 3;
        let id = Store::new(&conn).save(&game).unwrap();

        let recovered = Store::new(&conn).get(id).unwrap().unwrap();

        assert_eq!(recovered.summary.mode, Mode::Time);
        assert_eq!(recovered.summary.depth, 5000);
        assert_eq!(recovered.summary.multipv, 3);
    }

    #[test]
    fn partida_em_modo_time_coexiste_com_depth_na_mesma_pgn() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        let mut depth_game = partida_exemplo();
        depth_game.pgn = "1. d4 d5".to_string();
        store.save(&depth_game).unwrap();

        let mut time_game = partida_exemplo();
        time_game.pgn = "1. d4 d5".to_string();
        time_game.mode = Mode::Time;
        time_game.depth = 3000;
        store.save(&time_game).unwrap();

        let lista = store.list().unwrap();
        assert_eq!(lista.len(), 2, "mesma PGN, modos diferentes = 2 entradas");
    }

    #[test]
    fn perfil_automatico_coexiste_com_tempo_manual_na_mesma_pgn() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        let mut manual = partida_exemplo();
        manual.mode = Mode::Time;
        manual.depth = 1500;
        store.save(&manual).unwrap();

        let mut automatico = partida_exemplo();
        automatico.mode = Mode::Time;
        automatico.analysis_kind = "auto-fast".to_string();
        automatico.depth = 1500;
        store.save(&automatico).unwrap();

        let lista = store.list().unwrap();
        assert_eq!(lista.len(), 2);
        assert!(lista.iter().any(|game| game.analysis_kind == "auto-fast"));
        assert!(lista.iter().any(|game| game.analysis_kind == "manual"));
    }

    #[test]
    fn partida_salva_e_recuperada_com_pgn_e_revisao_intactos() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        let id = store.save(&partida_exemplo()).unwrap();

        let game = store.get(id).unwrap().unwrap();

        assert_eq!(game.summary.white, "Brancas");
        assert_eq!(game.summary.black, "Pretas");
        assert_eq!(game.summary.result, "1-0");
        assert_eq!(game.summary.plies, 2);
        assert_eq!(game.summary.engine_tier, "balanced");
        assert_eq!(game.summary.accuracy_white, 98.5);
        assert_eq!(game.pgn, "1. e4 e5");
        assert_eq!(game.review_json, r#"{"positions":[],"moves":[]}"#);
    }

    #[test]
    fn lista_devolve_mais_recentes_primeiro() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        let mut antiga = partida_exemplo();
        antiga.white = "Antiga".to_string();
        store.save(&antiga).unwrap();
        let mut recente = partida_exemplo();
        recente.pgn = "1. d4 d5".to_string();
        recente.white = "Recente".to_string();
        store.save(&recente).unwrap();

        let lista = store.list().unwrap();

        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].white, "Recente");
        assert_eq!(lista[1].white, "Antiga");
    }

    #[test]
    fn reanalise_com_mesma_chave_substitui_entrada() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        store.save(&partida_exemplo()).unwrap();
        let mut nova = partida_exemplo();
        nova.accuracy_white = 100.0;
        nova.review_json = r#"{"nova":true}"#.to_string();

        let id = store.save(&nova).unwrap();

        let lista = store.list().unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].accuracy_white, 100.0);
        let game = store.get(id).unwrap().unwrap();
        assert_eq!(game.review_json, r#"{"nova":true}"#);
    }

    #[test]
    fn delete_remove_partida_do_store() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        let id = store.save(&partida_exemplo()).unwrap();

        store.remove(id).unwrap();

        assert_eq!(store.get(id).unwrap(), None);
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn clear_games_esvazia_store_de_partidas() {
        let conn = open_memory().unwrap();
        let store = Store::new(&conn);
        store.save(&partida_exemplo()).unwrap();
        let mut outra = partida_exemplo();
        outra.pgn = "1. d4 d5".to_string();
        store.save(&outra).unwrap();
        assert_eq!(store.list().unwrap().len(), 2, "pré-condição");

        store.clear().unwrap();

        assert!(store.list().unwrap().is_empty());
    }
}
