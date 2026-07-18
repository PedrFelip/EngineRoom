//! Persistência SQLite: cache de posições avaliadas pelo engine e store de
//! partidas revisadas.
//!
//! O cache é chaveado por (fen, depth, multipv) — chave exata: uma análise só
//! reutiliza uma posição se os três parâmetros baterem. `lines_json` guarda as
//! linhas candidatas no formato do frontend (`[{multipv, cp, pv, san}]`).

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Conexão SQLite compartilhada pelos comandos Tauri.
pub struct DbState(pub Mutex<Connection>);

/// Avaliação cacheada de uma posição. A chave (fen, depth, multipv) é conhecida
/// por quem consulta, então só o payload volta.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPosition {
    pub cp: i32,
    pub lines_json: String,
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS position_cache (
            fen TEXT NOT NULL,
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            cp INTEGER NOT NULL,
            lines_json TEXT NOT NULL,
            PRIMARY KEY (fen, depth, multipv)
        );
        CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pgn TEXT NOT NULL,
            white TEXT NOT NULL,
            black TEXT NOT NULL,
            result TEXT NOT NULL,
            plies INTEGER NOT NULL,
            engine_tier TEXT NOT NULL,
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            accuracy_white REAL NOT NULL,
            accuracy_black REAL NOT NULL,
            review_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (pgn, depth, multipv)
        );",
    )
    .map_err(|e| e.to_string())
}

pub fn open_memory() -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

/// Abre (criando, se preciso) o banco em disco. O diretório pai deve existir.
pub fn open_file(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

#[tauri::command]
pub fn cache_get(
    state: tauri::State<'_, DbState>,
    fen: &str,
    depth: u32,
    multipv: u32,
) -> Result<Option<CachedPosition>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    cache_lookup(&conn, fen, depth, multipv)
}

#[tauri::command]
pub fn cache_put(
    state: tauri::State<'_, DbState>,
    fen: &str,
    depth: u32,
    multipv: u32,
    cp: i32,
    lines_json: &str,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    cache_store(&conn, fen, depth, multipv, cp, lines_json)
}

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

fn store_game(conn: &Connection, game: &NewGame) -> Result<i64, String> {
    conn.execute(
        "INSERT OR REPLACE INTO games
            (pgn, white, black, result, plies, engine_tier, depth, multipv,
             accuracy_white, accuracy_black, review_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        (
            &game.pgn,
            &game.white,
            &game.black,
            &game.result,
            game.plies,
            &game.engine_tier,
            game.depth,
            game.multipv,
            game.accuracy_white,
            game.accuracy_black,
            &game.review_json,
        ),
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn lookup_game(conn: &Connection, id: i64) -> Result<Option<StoredGame>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, white, black, result, plies, engine_tier, depth, multipv,
                    accuracy_white, accuracy_black, created_at, pgn, review_json
             FROM games WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query((id,)).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(stored_from_row(row)?)),
        None => Ok(None),
    }
}

fn stored_from_row(row: &rusqlite::Row<'_>) -> Result<StoredGame, String> {
    Ok(StoredGame {
        summary: GameSummary {
            id: row.get(0).map_err(|e| e.to_string())?,
            white: row.get(1).map_err(|e| e.to_string())?,
            black: row.get(2).map_err(|e| e.to_string())?,
            result: row.get(3).map_err(|e| e.to_string())?,
            plies: row.get(4).map_err(|e| e.to_string())?,
            engine_tier: row.get(5).map_err(|e| e.to_string())?,
            depth: row.get(6).map_err(|e| e.to_string())?,
            multipv: row.get(7).map_err(|e| e.to_string())?,
            accuracy_white: row.get(8).map_err(|e| e.to_string())?,
            accuracy_black: row.get(9).map_err(|e| e.to_string())?,
            created_at: row.get(10).map_err(|e| e.to_string())?,
        },
        pgn: row.get(11).map_err(|e| e.to_string())?,
        review_json: row.get(12).map_err(|e| e.to_string())?,
    })
}

fn cache_store(
    conn: &Connection,
    fen: &str,
    depth: u32,
    multipv: u32,
    cp: i32,
    lines_json: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO position_cache (fen, depth, multipv, cp, lines_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (fen, depth, multipv, cp, lines_json),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn cache_lookup(
    conn: &Connection,
    fen: &str,
    depth: u32,
    multipv: u32,
) -> Result<Option<CachedPosition>, String> {
    let mut stmt = conn
        .prepare("SELECT cp, lines_json FROM position_cache WHERE fen = ?1 AND depth = ?2 AND multipv = ?3")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query((fen, depth, multipv)).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(CachedPosition {
            cp: row.get(0).map_err(|e| e.to_string())?,
            lines_json: row.get(1).map_err(|e| e.to_string())?,
        })),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

    #[test]
    fn cache_put_depois_get_devolve_posicao_armazenada() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, 20, 1, 35, LINES).unwrap();

        let hit = cache_lookup(&conn, FEN, 20, 1).unwrap();

        assert_eq!(
            hit,
            Some(CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
            })
        );
    }

    #[test]
    fn cache_get_em_posicao_desconhecida_devolve_none() {
        let conn = open_memory().unwrap();

        assert_eq!(cache_lookup(&conn, FEN, 20, 1).unwrap(), None);
    }

    #[test]
    fn cache_so_reutiliza_com_depth_e_multipv_exatos() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, 20, 1, 35, LINES).unwrap();

        assert_eq!(cache_lookup(&conn, FEN, 15, 1).unwrap(), None, "depth menor");
        assert_eq!(cache_lookup(&conn, FEN, 25, 1).unwrap(), None, "depth maior");
        assert_eq!(cache_lookup(&conn, FEN, 20, 3).unwrap(), None, "multipv diferente");
    }

    #[test]
    fn cache_put_repetido_sobrescreve_entrada() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, 20, 1, 35, LINES).unwrap();
        cache_store(&conn, FEN, 20, 1, 42, "[]").unwrap();

        let hit = cache_lookup(&conn, FEN, 20, 1).unwrap().unwrap();

        assert_eq!(hit.cp, 42);
        assert_eq!(hit.lines_json, "[]");
    }

    fn partida_exemplo() -> NewGame {
        NewGame {
            pgn: "1. e4 e5".to_string(),
            white: "Brancas".to_string(),
            black: "Pretas".to_string(),
            result: "1-0".to_string(),
            plies: 2,
            engine_tier: "balanced".to_string(),
            depth: 20,
            multipv: 1,
            accuracy_white: 98.5,
            accuracy_black: 91.0,
            review_json: r#"{"positions":[],"moves":[]}"#.to_string(),
        }
    }

    #[test]
    fn partida_salva_e_recuperada_com_pgn_e_revisao_intactos() {
        let conn = open_memory().unwrap();
        let id = store_game(&conn, &partida_exemplo()).unwrap();

        let game = lookup_game(&conn, id).unwrap().unwrap();

        assert_eq!(game.summary.white, "Brancas");
        assert_eq!(game.summary.black, "Pretas");
        assert_eq!(game.summary.result, "1-0");
        assert_eq!(game.summary.plies, 2);
        assert_eq!(game.summary.engine_tier, "balanced");
        assert_eq!(game.summary.accuracy_white, 98.5);
        assert_eq!(game.pgn, "1. e4 e5");
        assert_eq!(game.review_json, r#"{"positions":[],"moves":[]}"#);
    }
}
