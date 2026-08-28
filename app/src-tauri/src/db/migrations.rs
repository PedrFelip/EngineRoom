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
    migrate_games_analysis_kind(conn)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cache::Cache, games::Store, mode::Mode, open_memory, stats::Stats};
    use rusqlite::Connection;

    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

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
        let hit = Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap();
        assert_eq!(
            hit,
            Some(crate::db::cache::CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
                reached_depth: 20,
            })
        );

        // Migração é idempotente: rodar de novo não quebra nem duplica.
        migrate(&conn).unwrap();
        let hit2 = Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap();
        assert_eq!(hit, hit2);
    }

    #[test]
    fn migracao_reached_depth_preserva_depth_e_descarta_time_legacy() {
        // Banco intermediário: tem `mode` mas ainda não tem `reached_depth`.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE position_cache (
                fen TEXT NOT NULL,
                mode TEXT NOT NULL DEFAULT 'depth',
                depth INTEGER NOT NULL,
                multipv INTEGER NOT NULL,
                cp INTEGER NOT NULL,
                lines_json TEXT NOT NULL,
                PRIMARY KEY (fen, mode, depth, multipv)
             );",
        )
        .unwrap();
        // Linha depth: depth=20 → deve sobreviver com reached_depth=20.
        conn.execute(
            "INSERT INTO position_cache (fen, mode, depth, multipv, cp, lines_json)
             VALUES (?1, 'depth', 20, 1, 35, ?2)",
            (FEN, LINES),
        )
        .unwrap();
        // Linha time: movetimeMs=5000 → descartada (não sabemos o reached).
        conn.execute(
            "INSERT INTO position_cache (fen, mode, depth, multipv, cp, lines_json)
             VALUES (?1, 'time', 5000, 1, 42, '[]')",
            (FEN,),
        )
        .unwrap();

        migrate(&conn).unwrap();

        // Depth row preservada com reached_depth=20.
        let depth_hit = Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap();
        assert_eq!(
            depth_hit,
            Some(crate::db::cache::CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
                reached_depth: 20,
            })
        );
        // Time row descartada.
        assert_eq!(
            Cache::new(&conn).lookup(FEN, Mode::Time, 5000, 1).unwrap(),
            None
        );

        // Idempotente.
        migrate(&conn).unwrap();
        assert_eq!(
            Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap(),
            depth_hit
        );
    }

    /// Banco "antigo" de `games` (pré-migração `mode`): sem a coluna `mode`,
    /// `UNIQUE (pgn, depth, multipv)`, populado com uma linha legacy.
    fn banco_com_games_legacy() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE games (
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
        .unwrap();
        conn.execute(
            "INSERT INTO games (pgn, white, black, result, plies, engine_tier, depth, multipv,
                                accuracy_white, accuracy_black, review_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                "1. e4 e5",
                "Brancas",
                "Pretas",
                "1-0",
                2,
                "balanced",
                20,
                1,
                98.5,
                91.0,
                r#"{"positions":[]}"#,
            ],
        )
        .unwrap();
        conn
    }

    #[test]
    fn migracao_games_mode_preserva_dados_legacy_sem_coluna_mode() {
        let conn = banco_com_games_legacy();

        // Pré-migração: coluna `mode` ausente.
        assert!(!has_column(&conn, "games", "mode").unwrap());

        migrate(&conn).unwrap();

        // Pós-migração: `mode` presente, linha legacy sobrevive com mode='depth'.
        assert!(has_column(&conn, "games", "mode").unwrap());
        let lista = Store::new(&conn).list().unwrap();
        assert_eq!(lista.len(), 1, "linha legacy deve sobreviver à migração");
        assert_eq!(
            lista[0].mode,
            Mode::Depth,
            "default da migração é mode='depth'"
        );
        assert_eq!(lista[0].white, "Brancas");
        assert_eq!(lista[0].depth, 20);
        assert_eq!(lista[0].analysis_kind, "manual");
    }

    #[test]
    fn migracao_games_mode_eh_idempotente() {
        let conn = banco_com_games_legacy();

        migrate(&conn).unwrap();
        // Segunda chamada: `mode` já existe → no-op, sem duplicar.
        migrate(&conn).unwrap();

        let lista = Store::new(&conn).list().unwrap();
        assert_eq!(lista.len(), 1, "idempotente: sem duplicação");
        assert_eq!(lista[0].white, "Brancas");
    }

    #[test]
    fn migrate_em_schema_atual_eh_noop() {
        // open_memory() já roda migrate() uma vez → schema no estado latest.
        let conn = open_memory().unwrap();

        // Rodar migrate() de novo (como ocorre a cada startup) não deve falhar.
        migrate(&conn).unwrap();

        // Tabelas acessíveis e vazias.
        let stats = Stats::new(&conn).compute().unwrap();
        assert_eq!(stats.cache_bytes, 0);
        assert_eq!(stats.games_bytes, 0);
        assert!(Store::new(&conn).list().unwrap().is_empty());
    }
}
