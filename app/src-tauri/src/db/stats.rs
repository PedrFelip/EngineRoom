//! Estatísticas de armazenamento usadas pelas tabelas do app, expostas ao
//! frontend para o painel de "Armazenamento" nas Configurações.

use crate::db::DbState;
use rusqlite::Connection;

/// Totais de armazenamento. `db_bytes` é o tamanho do arquivo `engineroom.db`
/// em disco (0 quando calculado sobre um banco in-memory, em testes);
/// `cache_bytes` e `games_bytes` são a soma dos comprimentos das colunas de
/// texto de cada tabela — aproximam o quanto cada tabela "pesa" sem depender
/// de detalhes de paginação do SQLite.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub cache_bytes: u64,
    pub games_bytes: u64,
    pub db_bytes: u64,
}

/// Vista sobre uma [`Connection`] para cálculo de estatísticas de armazenamento.
pub struct Stats<'a> {
    conn: &'a Connection,
}

impl<'a> Stats<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Soma os comprimentos das colunas de texto de cada tabela. Não preenche
    /// `db_bytes` (resolvido pelo comando a partir do arquivo em disco).
    pub fn compute(&self) -> Result<StorageStats, String> {
        let cache_bytes: u64 = self
            .conn
            .query_row(
                "SELECT COALESCE(SUM(LENGTH(fen) + LENGTH(source_mode) + LENGTH(lines_json)), 0)
                 FROM position_cache",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        let games_bytes: u64 = self
            .conn
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
}

/// Estatísticas de armazenamento para o painel de Configurações. `db_bytes` é o
/// tamanho do arquivo `engineroom.db` em disco (resolvido via `app_data_dir`);
/// `cache_bytes` e `games_bytes` somam os comprimentos das colunas de texto.
#[tauri::command]
pub fn storage_stats(
    state: tauri::State<'_, DbState>,
    app: tauri::AppHandle,
) -> Result<StorageStats, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stats = Stats::new(&conn).compute()?;
    use tauri::Manager;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let db_bytes = std::fs::metadata(dir.join("engineroom.db"))
        .map(|m| m.len())
        .unwrap_or(0);
    stats.db_bytes = db_bytes;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{cache::Cache, games::NewGame, games::Store, mode::Mode, open_memory};

    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

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
    fn storage_stats_reporta_bytes_das_tabelas() {
        let conn = open_memory().unwrap();
        let vazio = Stats::new(&conn).compute().unwrap();
        assert_eq!(vazio.cache_bytes, 0, "banco vazio: cache em zero bytes");
        assert_eq!(vazio.games_bytes, 0, "banco vazio: games em zero bytes");

        Cache::new(&conn)
            .store(FEN, Mode::Depth, 20, 1, 20, 35, LINES)
            .unwrap();
        Store::new(&conn).save(&partida_exemplo()).unwrap();

        let populado = Stats::new(&conn).compute().unwrap();
        assert!(
            populado.cache_bytes >= (FEN.len() + LINES.len()) as u64,
            "cache_bytes deve refletir ao menos fen + lines_json: got {}",
            populado.cache_bytes
        );
        assert!(
            populado.games_bytes
                >= "1. e4 e5".len() as u64 + r#"{"positions":[],"moves":[]}"#.len() as u64,
            "games_bytes deve refletir ao menos pgn + review_json: got {}",
            populado.games_bytes
        );
    }

    #[test]
    fn storage_stats_zera_apos_clear_de_ambas_as_tabelas() {
        let conn = open_memory().unwrap();
        Cache::new(&conn)
            .store(FEN, Mode::Depth, 20, 1, 20, 35, LINES)
            .unwrap();
        Store::new(&conn).save(&partida_exemplo()).unwrap();

        // Pré-condição: ambas as tabelas com bytes > 0.
        let antes = Stats::new(&conn).compute().unwrap();
        assert!(antes.cache_bytes > 0, "pré-condição: cache populado");
        assert!(antes.games_bytes > 0, "pré-condição: games populado");

        Cache::new(&conn).clear().unwrap();
        Store::new(&conn).clear().unwrap();

        let depois = Stats::new(&conn).compute().unwrap();
        assert_eq!(depois.cache_bytes, 0, "cache_bytes volta a zero após clear");
        assert_eq!(depois.games_bytes, 0, "games_bytes volta a zero após clear");
    }
}
