//! Schema migrations: rodam a cada startup (idempotentes). Padrão: para
//! adicionar uma coluna, escreva um novo `migrate_*` gated em `has_column`
//! (via `PRAGMA table_info`) — nunca edite o `CREATE TABLE` histórico.

use rusqlite::Connection;

/// Aplica todas as migrações na ordem. Idempotente: no-op quando o schema já
/// está atualizado. Chamado por [`crate::db::open_file`] e
/// [`crate::db::open_memory`].
pub(super) fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS position_cache (
            fen TEXT NOT NULL,
            reached_depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            source_mode TEXT NOT NULL,
            source_value INTEGER NOT NULL,
            cp INTEGER NOT NULL,
            lines_json TEXT NOT NULL,
            PRIMARY KEY (fen, reached_depth, multipv)
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
            analysis_kind TEXT NOT NULL DEFAULT 'manual',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            accuracy_white REAL NOT NULL,
            accuracy_black REAL NOT NULL,
            review_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (pgn, analysis_kind, mode, depth, multipv)
        );",
    )
    .map_err(|e| e.to_string())?;
    migrate_position_cache_mode(conn)?;
    migrate_position_cache_reached_depth(conn)?;
    migrate_games_mode(conn)?;
    migrate_games_analysis_kind(conn)?;
    migrate_games_list_index(conn)
}

/// `true` se a coluna existe na tabela (via `PRAGMA table_info`).
fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(()).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Migração do `position_cache` para incluir a coluna `mode` (depth | time) na
/// chave primária. Idempotente: se a tabela já está no schema unificado
/// (`reached_depth` presente) ou já tem `mode`, é no-op; se não, recria a
/// tabela copiando as linhas antigas (que recebem `mode='depth'`).
fn migrate_position_cache_mode(conn: &Connection) -> Result<(), String> {
    if has_column(conn, "position_cache", "reached_depth")? {
        return Ok(());
    }
    if has_column(conn, "position_cache", "mode")? {
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

/// Migração do `position_cache` para o schema unificado: chave por
/// (fen, reached_depth, multipv), com `source_mode`/`source_value` como
/// metadata do contexto original. Linhas legacy de depth são preservadas com
/// `reached_depth = depth` (sob `go depth N`, reached == pedido); linhas legacy
/// de time são descartadas (não sabemos o reached_depth atingido). Idempotente.
fn migrate_position_cache_reached_depth(conn: &Connection) -> Result<(), String> {
    if has_column(conn, "position_cache", "reached_depth")? {
        return Ok(());
    }

    conn.execute_batch(
        "ALTER TABLE position_cache RENAME TO position_cache_old;
         CREATE TABLE position_cache (
            fen TEXT NOT NULL,
            reached_depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            source_mode TEXT NOT NULL,
            source_value INTEGER NOT NULL,
            cp INTEGER NOT NULL,
            lines_json TEXT NOT NULL,
            PRIMARY KEY (fen, reached_depth, multipv)
         );
         INSERT INTO position_cache
            (fen, reached_depth, multipv, source_mode, source_value, cp, lines_json)
         SELECT fen, depth, multipv, 'depth', depth, cp, lines_json
         FROM position_cache_old WHERE mode = 'depth';
         DROP TABLE position_cache_old;",
    )
    .map_err(|e| e.to_string())
}

/// Migração análoga para a tabela `games`: adiciona `mode` à chave UNIQUE,
/// permitindo reanalisar a mesma PGN em modos diferentes (depth vs time).
fn migrate_games_mode(conn: &Connection) -> Result<(), String> {
    if has_column(conn, "games", "mode")? {
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

/// Adiciona a estratégia da revisão à tabela de partidas e à chave UNIQUE.
/// Revisões existentes continuam sendo controles manuais.
fn migrate_games_analysis_kind(conn: &Connection) -> Result<(), String> {
    if has_column(conn, "games", "analysis_kind")? {
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
            analysis_kind TEXT NOT NULL DEFAULT 'manual',
            depth INTEGER NOT NULL,
            multipv INTEGER NOT NULL,
            accuracy_white REAL NOT NULL,
            accuracy_black REAL NOT NULL,
            review_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (pgn, analysis_kind, mode, depth, multipv)
         );
         INSERT INTO games (id, pgn, white, black, result, plies, engine_tier, mode,
                analysis_kind, depth, multipv, accuracy_white, accuracy_black,
                review_json, created_at)
            SELECT id, pgn, white, black, result, plies, engine_tier, mode,
                'manual', depth, multipv, accuracy_white, accuracy_black,
                review_json, created_at
            FROM games_old;
         DROP TABLE games_old;",
    )
    .map_err(|e| e.to_string())
}

/// Índice para a listagem paginada do histórico, ordenada por data e id.
fn migrate_games_list_index(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS games_created_at_id ON games (created_at DESC, id DESC);",
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
#[cfg(test)]
#[path = "migrations/tests.rs"]
mod tests;
