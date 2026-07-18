//! Persistência SQLite: cache de posições avaliadas pelo engine e store de
//! partidas revisadas.
//!
//! O cache é chaveado por (fen, mode, depth, multipv) — chave exata: uma
//! análise só reutiliza uma posição se os quatro parâmetros baterem. `mode`
//! é `"depth"` (profundidade fixa, `depth` = ply alvo) ou `"time"` (tempo
//! fixo por lance, `depth` = milissegundos). `lines_json` guarda as linhas
//! candidatas no formato do frontend (`[{multipv, cp, pv, san}]`).

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

/// Totais de armazenamento usados pelas tabelas do app, expostos ao
/// frontend para o painel de "Armazenamento" nas Configurações.
/// `db_bytes` é o tamanho do arquivo `engineroom.db` em disco (0 em testes
/// in-memory); `cache_bytes` e `games_bytes` são a soma dos comprimentos das
/// colunas de texto de cada tabela.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub cache_bytes: u64,
    pub games_bytes: u64,
    pub db_bytes: u64,
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS position_cache (
            fen TEXT NOT NULL,
            mode TEXT NOT NULL DEFAULT 'depth',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            cp INTEGER NOT NULL,
            lines_json TEXT NOT NULL,
            PRIMARY KEY (fen, mode, depth, multipv)
        );
        CREATE TABLE IF NOT EXISTS games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pgn TEXT NOT NULL,
            white TEXT NOT NULL,
            black TEXT NOT NULL,
            result TEXT NOT NULL,
            plies INTEGER NOT NULL,
            engine_tier TEXT NOT NULL,
            mode TEXT NOT NULL DEFAULT 'depth',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            accuracy_white REAL NOT NULL,
            accuracy_black REAL NOT NULL,
            review_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (pgn, mode, depth, multipv)
        );",
    )
    .map_err(|e| e.to_string())?;
    migrate_position_cache_mode(conn)?;
    migrate_games_mode(conn)
}

/// Migração do `position_cache` para incluir a coluna `mode` (depth | time) na
/// chave primária. Idempotente: detecta (via `PRAGMA table_info`) se a coluna
/// já existe; se não, recria a tabela com o novo esquema copiando as linhas
/// antigas (que recebem `mode='depth'` por herança do DEFAULT).
fn migrate_position_cache_mode(conn: &Connection) -> Result<(), String> {
    let has_mode = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(position_cache)")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(()).map_err(|e| e.to_string())?;
        let mut found = false;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let name: String = row.get(1).map_err(|e| e.to_string())?;
            if name == "mode" {
                found = true;
                break;
            }
        }
        found
    };
    if has_mode {
        return Ok(());
    }

    conn.execute_batch(
        "ALTER TABLE position_cache RENAME TO position_cache_old;
         CREATE TABLE position_cache (
            fen TEXT NOT NULL,
            mode TEXT NOT NULL DEFAULT 'depth',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            cp INTEGER NOT NULL,
            lines_json TEXT NOT NULL,
            PRIMARY KEY (fen, mode, depth, multipv)
         );
         INSERT INTO position_cache (fen, mode, depth, multipv, cp, lines_json)
            SELECT fen, 'depth', depth, multipv, cp, lines_json FROM position_cache_old;
         DROP TABLE position_cache_old;",
    )
    .map_err(|e| e.to_string())
}

/// Migração análoga para a tabela `games`: adiciona `mode` à chave UNIQUE,
/// permitindo reanalisar a mesma PGN em modos diferentes (depth vs time).
fn migrate_games_mode(conn: &Connection) -> Result<(), String> {
    let has_mode = {
        let mut stmt = conn
            .prepare("PRAGMA table_info(games)")
            .map_err(|e| e.to_string())?;
        let mut rows = stmt.query(()).map_err(|e| e.to_string())?;
        let mut found = false;
        while let Some(row) = rows.next().map_err(|e| e.to_string())? {
            let name: String = row.get(1).map_err(|e| e.to_string())?;
            if name == "mode" {
                found = true;
                break;
            }
        }
        found
    };
    if has_mode {
        return Ok(());
    }

    conn.execute_batch(
        "ALTER TABLE games RENAME TO games_old;
         CREATE TABLE games (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pgn TEXT NOT NULL,
            white TEXT NOT NULL,
            black TEXT NOT NULL,
            result TEXT NOT NULL,
            plies INTEGER NOT NULL,
            engine_tier TEXT NOT NULL,
            mode TEXT NOT NULL DEFAULT 'depth',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            accuracy_white REAL NOT NULL,
            accuracy_black REAL NOT NULL,
            review_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (pgn, mode, depth, multipv)
         );
         INSERT INTO games (id, pgn, white, black, result, plies, engine_tier, mode,
                depth, multipv, accuracy_white, accuracy_black, review_json, created_at)
            SELECT id, pgn, white, black, result, plies, engine_tier, 'depth',
                depth, multipv, accuracy_white, accuracy_black, review_json, created_at
            FROM games_old;
         DROP TABLE games_old;",
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
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
    mode: String,
    depth: u32,
    multipv: u32,
) -> Result<Option<CachedPosition>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    cache_lookup(&conn, fen, &mode, depth, multipv)
}

#[tauri::command]
pub fn cache_put(
    state: tauri::State<'_, DbState>,
    fen: &str,
    mode: String,
    depth: u32,
    multipv: u32,
    cp: i32,
    lines_json: &str,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    cache_store(&conn, fen, &mode, depth, multipv, cp, lines_json)
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
    pub mode: String,
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
    pub mode: String,
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
            (pgn, white, black, result, plies, engine_tier, mode, depth, multipv,
             accuracy_white, accuracy_black, review_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        (
            &game.pgn,
            &game.white,
            &game.black,
            &game.result,
            game.plies,
            &game.engine_tier,
            &game.mode,
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

const SUMMARY_COLS: &str = "id, white, black, result, plies, engine_tier, mode, depth, multipv,
        accuracy_white, accuracy_black, created_at";

/// Mapeia as colunas (ordem de SUMMARY_COLS) para GameSummary.
fn summary_from_row(row: &rusqlite::Row<'_>) -> Result<GameSummary, String> {
    Ok(GameSummary {
        id: row.get(0).map_err(|e| e.to_string())?,
        white: row.get(1).map_err(|e| e.to_string())?,
        black: row.get(2).map_err(|e| e.to_string())?,
        result: row.get(3).map_err(|e| e.to_string())?,
        plies: row.get(4).map_err(|e| e.to_string())?,
        engine_tier: row.get(5).map_err(|e| e.to_string())?,
        mode: row.get(6).map_err(|e| e.to_string())?,
        depth: row.get(7).map_err(|e| e.to_string())?,
        multipv: row.get(8).map_err(|e| e.to_string())?,
        accuracy_white: row.get(9).map_err(|e| e.to_string())?,
        accuracy_black: row.get(10).map_err(|e| e.to_string())?,
        created_at: row.get(11).map_err(|e| e.to_string())?,
    })
}

fn list_games(conn: &Connection) -> Result<Vec<GameSummary>, String> {
    let mut stmt = conn
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

fn lookup_game(conn: &Connection, id: i64) -> Result<Option<StoredGame>, String> {
    let mut stmt = conn
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

fn remove_game(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM games WHERE id = ?1", (id,))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn clear_cache(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM position_cache", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn clear_games(conn: &Connection) -> Result<(), String> {
    conn.execute("DELETE FROM games", []).map_err(|e| e.to_string())?;
    Ok(())
}

fn storage_stats(conn: &Connection) -> Result<StorageStats, String> {
    let cache_bytes: u64 = conn
        .query_row(
            "SELECT COALESCE(SUM(LENGTH(fen) + LENGTH(mode) + LENGTH(lines_json)), 0)
             FROM position_cache",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let games_bytes: u64 = conn
        .query_row(
            "SELECT COALESCE(
                SUM(LENGTH(pgn) + LENGTH(white) + LENGTH(black) + LENGTH(result)
                    + LENGTH(engine_tier) + LENGTH(mode) + LENGTH(review_json)),
                0
             )
             FROM games",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(StorageStats {
        cache_bytes,
        games_bytes,
        db_bytes: 0,
    })
}

#[tauri::command]
pub fn games_save(state: tauri::State<'_, DbState>, game: NewGame) -> Result<i64, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    store_game(&conn, &game)
}

#[tauri::command]
pub fn games_list(state: tauri::State<'_, DbState>) -> Result<Vec<GameSummary>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    list_games(&conn)
}

#[tauri::command]
pub fn games_get(state: tauri::State<'_, DbState>, id: i64) -> Result<Option<StoredGame>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    lookup_game(&conn, id)
}

#[tauri::command]
pub fn games_delete(state: tauri::State<'_, DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    remove_game(&conn, id)
}

#[tauri::command]
pub fn cache_clear(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    clear_cache(&conn)
}

#[tauri::command]
pub fn games_clear(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    clear_games(&conn)
}

/// Estatísticas de armazenamento para o painel de Configurações.
/// `db_bytes` é o tamanho do arquivo `engineroom.db` em disco (resolvido via
/// `app_data_dir`); `cache_bytes` e `games_bytes` somam os comprimentos das
/// colunas de texto — aproximam o quanto cada tabela "pesa" sem depender de
/// detalhes de paginação do SQLite.
#[tauri::command]
pub fn storage_stats(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<StorageStats, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stats = storage_stats(&conn)?;
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_bytes = std::fs::metadata(&dir.join("engineroom.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    stats.db_bytes = db_bytes;
    Ok(stats)
}

fn stored_from_row(row: &rusqlite::Row<'_>) -> Result<StoredGame, String> {
    Ok(StoredGame {
        summary: summary_from_row(row)?,
        pgn: row.get(12).map_err(|e| e.to_string())?,
        review_json: row.get(13).map_err(|e| e.to_string())?,
    })
}

fn cache_store(
    conn: &Connection,
    fen: &str,
    mode: &str,
    depth: u32,
    multipv: u32,
    cp: i32,
    lines_json: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO position_cache (fen, mode, depth, multipv, cp, lines_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (fen, mode, depth, multipv, cp, lines_json),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn cache_lookup(
    conn: &Connection,
    fen: &str,
    mode: &str,
    depth: u32,
    multipv: u32,
) -> Result<Option<CachedPosition>, String> {
    let mut stmt = conn
        .prepare("SELECT cp, lines_json FROM position_cache WHERE fen = ?1 AND mode = ?2 AND depth = ?3 AND multipv = ?4")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query((fen, mode, depth, multipv))
        .map_err(|e| e.to_string())?;
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
        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();

        let hit = cache_lookup(&conn, FEN, "depth", 20, 1).unwrap();

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

        assert_eq!(cache_lookup(&conn, FEN, "depth", 20, 1).unwrap(), None);
    }

    #[test]
    fn cache_so_reutiliza_com_depth_e_multipv_exatos() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();

        assert_eq!(
            cache_lookup(&conn, FEN, "depth", 15, 1).unwrap(),
            None,
            "depth menor"
        );
        assert_eq!(
            cache_lookup(&conn, FEN, "depth", 25, 1).unwrap(),
            None,
            "depth maior"
        );
        assert_eq!(
            cache_lookup(&conn, FEN, "depth", 20, 3).unwrap(),
            None,
            "multipv diferente"
        );
    }

    #[test]
    fn cache_put_repetido_sobrescreve_entrada() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();
        cache_store(&conn, FEN, "depth", 20, 1, 42, "[]").unwrap();

        let hit = cache_lookup(&conn, FEN, "depth", 20, 1).unwrap().unwrap();

        assert_eq!(hit.cp, 42);
        assert_eq!(hit.lines_json, "[]");
    }

    #[test]
    fn cache_store_com_mode_time_faz_round_trip() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, "time", 5000, 1, 35, LINES).unwrap();

        let hit = cache_lookup(&conn, FEN, "time", 5000, 1).unwrap();

        assert_eq!(
            hit,
            Some(CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
            })
        );
    }

    #[test]
    fn cache_nao_cruza_modos_mesmo_com_mesmo_valor_numerico() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();

        // Mesma FEN, mesmo valor numérico (20), mas mode='time' → miss.
        let cruzado = cache_lookup(&conn, FEN, "time", 20, 1).unwrap();
        assert_eq!(cruzado, None);

        // E vice-versa: gravar em 'time' não polui 'depth'.
        cache_store(&conn, FEN, "time", 20, 1, 42, "[]").unwrap();
        let depth_hit = cache_lookup(&conn, FEN, "depth", 20, 1).unwrap().unwrap();
        assert_eq!(depth_hit.cp, 35, "depth preservado");
        let time_hit = cache_lookup(&conn, FEN, "time", 20, 1).unwrap().unwrap();
        assert_eq!(time_hit.cp, 42, "time gravado separadamente");
    }

    #[test]
    fn migracao_adiciona_coluna_mode_preservando_dados_de_depth() {
        // Banco "antigo": criado sem a coluna `mode`, populado antes da migrate().
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE position_cache (
                fen TEXT NOT NULL,
                depth INTEGER NOT NULL,
                multipv INTEGER NOT NULL,
                cp INTEGER NOT NULL,
                lines_json TEXT NOT NULL,
                PRIMARY KEY (fen, depth, multipv)
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO position_cache (fen, depth, multipv, cp, lines_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (FEN, 20, 1, 35, LINES),
        )
        .unwrap();

        // Roda a migração em cima do banco já populado.
        migrate(&conn).unwrap();

        // Dado antigo continua acessível como mode='depth'.
        let hit = cache_lookup(&conn, FEN, "depth", 20, 1).unwrap();
        assert_eq!(
            hit,
            Some(CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
            })
        );

        // Migração é idempotente: rodar de novo não quebra nem duplica.
        migrate(&conn).unwrap();
        let hit2 = cache_lookup(&conn, FEN, "depth", 20, 1).unwrap();
        assert_eq!(hit, hit2);
    }

    fn partida_exemplo() -> NewGame {
        NewGame {
            pgn: "1. e4 e5".to_string(),
            white: "Brancas".to_string(),
            black: "Pretas".to_string(),
            result: "1-0".to_string(),
            plies: 2,
            engine_tier: "balanced".to_string(),
            mode: "depth".to_string(),
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
        game.mode = "time".to_string();
        game.depth = 5000; // movetimeMs
        game.multipv = 3;
        let id = store_game(&conn, &game).unwrap();

        let recovered = lookup_game(&conn, id).unwrap().unwrap();

        assert_eq!(recovered.summary.mode, "time");
        assert_eq!(recovered.summary.depth, 5000);
        assert_eq!(recovered.summary.multipv, 3);
    }

    #[test]
    fn partida_em_modo_time_coexiste_com_depth_na_mesma_pgn() {
        let conn = open_memory().unwrap();
        let mut depth_game = partida_exemplo();
        depth_game.pgn = "1. d4 d5".to_string();
        store_game(&conn, &depth_game).unwrap();

        let mut time_game = partida_exemplo();
        time_game.pgn = "1. d4 d5".to_string();
        time_game.mode = "time".to_string();
        time_game.depth = 3000;
        store_game(&conn, &time_game).unwrap();

        let lista = list_games(&conn).unwrap();
        assert_eq!(lista.len(), 2, "mesma PGN, modos diferentes = 2 entradas");
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

    #[test]
    fn lista_devolve_mais_recentes_primeiro() {
        let conn = open_memory().unwrap();
        let mut antiga = partida_exemplo();
        antiga.white = "Antiga".to_string();
        store_game(&conn, &antiga).unwrap();
        let mut recente = partida_exemplo();
        recente.pgn = "1. d4 d5".to_string();
        recente.white = "Recente".to_string();
        store_game(&conn, &recente).unwrap();

        let lista = list_games(&conn).unwrap();

        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0].white, "Recente");
        assert_eq!(lista[1].white, "Antiga");
    }

    #[test]
    fn reanalise_com_mesma_chave_substitui_entrada() {
        let conn = open_memory().unwrap();
        store_game(&conn, &partida_exemplo()).unwrap();
        let mut nova = partida_exemplo();
        nova.accuracy_white = 100.0;
        nova.review_json = r#"{"nova":true}"#.to_string();

        let id = store_game(&conn, &nova).unwrap();

        let lista = list_games(&conn).unwrap();
        assert_eq!(lista.len(), 1);
        assert_eq!(lista[0].accuracy_white, 100.0);
        let game = lookup_game(&conn, id).unwrap().unwrap();
        assert_eq!(game.review_json, r#"{"nova":true}"#);
    }

    #[test]
    fn delete_remove_partida_do_store() {
        let conn = open_memory().unwrap();
        let id = store_game(&conn, &partida_exemplo()).unwrap();

        remove_game(&conn, id).unwrap();

        assert_eq!(lookup_game(&conn, id).unwrap(), None);
        assert!(list_games(&conn).unwrap().is_empty());
    }

    #[test]
    fn clear_cache_esvazia_tabela_de_posicoes() {
        let conn = open_memory().unwrap();
        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();
        cache_store(&conn, FEN, "time", 5000, 1, 35, LINES).unwrap();
        assert_eq!(
            cache_lookup(&conn, FEN, "depth", 20, 1).unwrap().is_some(),
            true,
            "pré-condição: cache populado"
        );

        clear_cache(&conn).unwrap();

        assert_eq!(
            cache_lookup(&conn, FEN, "depth", 20, 1).unwrap(),
            None,
            "entrada depth removida"
        );
        assert_eq!(
            cache_lookup(&conn, FEN, "time", 5000, 1).unwrap(),
            None,
            "entrada time removida"
        );
    }

    #[test]
    fn clear_games_esvazia_store_de_partidas() {
        let conn = open_memory().unwrap();
        store_game(&conn, &partida_exemplo()).unwrap();
        let mut outra = partida_exemplo();
        outra.pgn = "1. d4 d5".to_string();
        store_game(&conn, &outra).unwrap();
        assert_eq!(list_games(&conn).unwrap().len(), 2, "pré-condição");

        clear_games(&conn).unwrap();

        assert!(list_games(&conn).unwrap().is_empty());
    }

    #[test]
    fn storage_stats_reporta_bytes_das_tabelas() {
        let conn = open_memory().unwrap();
        let vazio = storage_stats(&conn).unwrap();
        assert_eq!(
            vazio.cache_bytes, 0,
            "banco vazio: cache em zero bytes"
        );
        assert_eq!(
            vazio.games_bytes, 0,
            "banco vazio: games em zero bytes"
        );

        cache_store(&conn, FEN, "depth", 20, 1, 35, LINES).unwrap();
        store_game(&conn, &partida_exemplo()).unwrap();

        let populado = storage_stats(&conn).unwrap();
        assert!(
            populado.cache_bytes >= (FEN.len() + LINES.len()) as u64,
            "cache_bytes deve refletir ao menos fen + lines_json: got {}",
            populado.cache_bytes
        );
        assert!(
            populado.games_bytes
                >= "1. e4 e5".len() as u64
                    + r#"{"positions":[],"moves":[]}"#.len() as u64,
            "games_bytes deve refletir ao menos pgn + review_json: got {}",
            populado.games_bytes
        );
    }
}
