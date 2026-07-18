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
}
