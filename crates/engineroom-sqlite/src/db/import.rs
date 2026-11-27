//! One-time import of reviews and cached analysis from earlier desktop apps.

use std::{error::Error, path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

/// Imports a legacy database once. Existing records take precedence on conflicts.
///
/// # Errors
/// Returns filesystem and SQLite errors. Failed imports can be retried and leave
/// the source unchanged.
pub fn import_legacy(
    destination: &Path,
    source: &Path,
    origin: &str,
) -> Result<(), Box<dyn Error>> {
    if !source.try_exists()? {
        return Ok(());
    }

    let mut destination = super::open_file(destination)?;
    destination.execute_batch(
        "CREATE TABLE IF NOT EXISTS imported_desktop_databases (origin TEXT PRIMARY KEY);",
    )?;
    if destination
        .query_row(
            "SELECT 1 FROM imported_desktop_databases WHERE origin = ?1",
            [origin],
            |_| Ok(()),
        )
        .optional()?
        .is_some()
    {
        return Ok(());
    }

    // SQLite backup includes committed WAL pages and allows upgrading the copy
    // without writing to the original installation.
    let directory = tempfile::tempdir()?;
    let snapshot_path = directory.path().join("legacy.db");
    let source = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut snapshot = Connection::open(&snapshot_path)?;
    let backup = rusqlite::backup::Backup::new(&source, &mut snapshot)?;
    match backup.step(-1)? {
        rusqlite::backup::StepResult::Done => {}
        _ => return Err("legacy database is busy".into()),
    }
    drop(backup);
    super::migrations::migrate(&snapshot)?;
    drop(snapshot);

    let snapshot_path = snapshot_path
        .to_str()
        .ok_or("legacy database snapshot path is not UTF-8")?;
    destination.execute("ATTACH DATABASE ?1 AS legacy", [snapshot_path])?;
    destination.busy_timeout(Duration::from_secs(5))?;
    let transaction = destination.transaction()?;
    if transaction.execute(
        "INSERT INTO imported_desktop_databases (origin) VALUES (?1)
         ON CONFLICT(origin) DO NOTHING",
        [origin],
    )? != 0
    {
        transaction.execute_batch(
            "INSERT INTO main.games
                (pgn, white, black, result, plies, engine_tier, mode, analysis_kind,
                 depth, multipv, accuracy_white, accuracy_black, review_json, created_at)
             SELECT pgn, white, black, result, plies, engine_tier, mode, analysis_kind,
                    depth, multipv, accuracy_white, accuracy_black, review_json, created_at
             FROM legacy.games WHERE true
             ON CONFLICT(pgn, analysis_kind, mode, depth, multipv) DO NOTHING;

             INSERT INTO main.position_cache
                (fen, reached_depth, multipv, source_mode, source_value, cp, lines_json)
             SELECT fen, reached_depth, multipv, source_mode, source_value, cp, lines_json
             FROM legacy.position_cache WHERE true
             ON CONFLICT(fen, reached_depth, multipv) DO NOTHING;",
        )?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::{cache::Cache, games::Store, mode::Mode},
        review::{core, repository::SqliteRepository},
    };

    #[tokio::test]
    async fn imports_committed_wal_and_legacy_schema_without_replacing_preview_history() {
        let directory = tempfile::tempdir().expect("test directory");
        let legacy_path = directory.path().join("legacy.db");
        let final_path = directory.path().join("engineroom.db");
        let legacy = Connection::open(&legacy_path).expect("open legacy fixture");
        legacy
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 CREATE TABLE position_cache (
                    fen TEXT NOT NULL, depth INTEGER NOT NULL, multipv INTEGER NOT NULL,
                    cp INTEGER NOT NULL, lines_json TEXT NOT NULL,
                    PRIMARY KEY (fen, depth, multipv)
                 );
                 INSERT INTO position_cache VALUES ('legacy-fen', 20, 1, 35, '[]');
                 CREATE TABLE games (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, pgn TEXT NOT NULL,
                    white TEXT NOT NULL, black TEXT NOT NULL, result TEXT NOT NULL,
                    plies INTEGER NOT NULL, engine_tier TEXT NOT NULL, depth INTEGER NOT NULL,
                    multipv INTEGER NOT NULL, accuracy_white REAL NOT NULL,
                    accuracy_black REAL NOT NULL, review_json TEXT NOT NULL,
                    created_at TEXT NOT NULL, UNIQUE (pgn, depth, multipv)
                 );
                 INSERT INTO games VALUES
                    (1, 'shared', 'Legacy', 'Black', '*', 1, 'manual', 20, 1, 0, 0, '{}', '2025-01-01'),
                    (2, 'legacy-only', 'White', 'Black', '*', 1, 'manual', 20, 1, 0, 0, '{}', '2025-01-02');",
            )
            .expect("create legacy schema and WAL records");
        let pgn = "1. e4 e5";
        let game = core::extract(pgn).expect("parse fixture game");
        let positions: Vec<_> = game
            .fens
            .iter()
            .map(|fen| core::terminal_raw(fen, 0))
            .collect();
        let analysis = core::build(&game, &positions).expect("build fixture review");
        legacy
            .execute(
                "UPDATE games SET pgn = ?1, plies = 2, review_json = ?2 WHERE id = 2",
                (
                    pgn,
                    serde_json::to_string(&analysis).expect("encode review"),
                ),
            )
            .expect("save review in the WAL");

        let destination = crate::db::open_file(&final_path).expect("open desktop fixture");
        destination
            .execute_batch(
                "INSERT INTO games
                    (pgn, white, black, result, plies, engine_tier, depth, multipv,
                     accuracy_white, accuracy_black, review_json)
                 VALUES ('shared', 'Rune', 'Black', '*', 1, 'manual', 20, 1, 0, 0, '{}');",
            )
            .expect("create existing review");
        drop(destination);

        import_legacy(&final_path, &legacy_path, "legacy").expect("import legacy database");
        let destination = crate::db::open_file(&final_path).expect("reopen migrated desktop");
        assert_eq!(
            Cache::new(&destination)
                .lookup("legacy-fen", Mode::Depth, 20, 1)
                .expect("read migrated cache")
                .expect("compatible cached position")
                .cp,
            35
        );
        let names = destination
            .prepare("SELECT white FROM games ORDER BY pgn")
            .expect("prepare history query")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query migrated history")
            .collect::<Result<Vec<_>, _>>()
            .expect("read migrated history");
        assert_eq!(names, ["White", "Rune"]);
        let imported_id = destination
            .query_row("SELECT id FROM games WHERE pgn = ?1", [pgn], |row| {
                row.get(0)
            })
            .expect("find imported review");
        let reopened = SqliteRepository::new(final_path.clone())
            .review_config(imported_id)
            .await
            .expect("reopen the imported analysis")
            .expect("imported analysis exists");
        assert_eq!(reopened.pgn, pgn);
        assert!(legacy.prepare("SELECT depth FROM position_cache").is_ok());

        Store::new(&destination).clear().expect("clear history");
        import_legacy(&final_path, &legacy_path, "legacy").expect("repeat import");
        let count: i64 = destination
            .query_row("SELECT count(*) FROM games", [], |row| row.get(0))
            .expect("count reviews");
        assert_eq!(count, 0, "one-time import must respect later deletions");
    }
}
