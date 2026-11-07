//! Estatísticas de armazenamento usadas pelas tabelas do app, expostas ao
//! frontend para o painel de "Armazenamento" nas Configurações.

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
    pub fn compute(&self) -> rusqlite::Result<StorageStats> {
        let cache_bytes: u64 = self.conn.query_row(
            "SELECT COALESCE(SUM(LENGTH(fen) + LENGTH(source_mode) + LENGTH(lines_json)), 0)
                 FROM position_cache",
            [],
            |row| row.get(0),
        )?;
        let games_bytes: u64 = self.conn.query_row(
            "SELECT COALESCE(
                    SUM(LENGTH(pgn) + LENGTH(white) + LENGTH(black) + LENGTH(result)
                        + LENGTH(engine_tier) + LENGTH(mode) + LENGTH(review_json)),
                    0
                 )
                 FROM games",
            [],
            |row| row.get(0),
        )?;
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

#[cfg(test)]
mod tests;
