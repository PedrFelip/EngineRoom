//! Cache de evidência contextual, indexado por uma chave opaca de contexto.
//!
//! Diferente de [`crate::db::cache::Cache`], que identifica uma avaliação apenas
//! pelo FEN, este cache permite ao chamador decidir o que faz uma evidência
//! equivalente: usualmente FEN + histórico conhecido + assinatura do motor +
//! escopo da busca + política de regras. A chave é opaca para este módulo —
//! nenhuma análise semântica é feita sobre ela — e o payload é JSON também
//! opaco, cabendo ao chamador validar o contexto e o digest antes de reusar.
//!
//! A semântica de cobertura espelha a do cache legado: pedidos de depth são
//! servidos por qualquer entry com `reached_depth >= P` (inclusive entries de
//! tempo), pedidos de time só por entries de tempo com `source_value >= T`, e
//! ambos exigem `multipv >= M`. O upsert preserva entries de tempo mais fortes
//! contra reescritas mais fracas. Nada aqui altera `position_cache` ou `games`.

use crate::db::mode::Mode;
use rusqlite::Connection;

/// Upsert condicional compartilhado entre [`EvidenceCache::store`] e
/// [`EvidenceCache::store_many`]. A PK `(context_key, reached_depth, multipv)`
/// coalesce entries equivalentes, mas não rebaixa cobertura temporal já obtida.
const INSERT_SQL: &str = "INSERT INTO evidence_cache
    (context_key, reached_depth, multipv, source_mode, source_value, cp, lines_json, payload_json)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT(context_key, reached_depth, multipv) DO UPDATE SET
    source_mode = excluded.source_mode,
    source_value = excluded.source_value,
    cp = excluded.cp,
    lines_json = excluded.lines_json,
    payload_json = excluded.payload_json
WHERE evidence_cache.source_mode != 'time'
   OR (excluded.source_mode = 'time' AND excluded.source_value >= evidence_cache.source_value)";

/// Evidência cacheada. A chave e a cobertura pedidas são conhecidas por quem
/// consulta; o que volta é o payload de avaliação e o contexto verificável.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedEvidence {
    pub cp: i32,
    pub lines_json: String,
    pub reached_depth: u32,
    /// JSON opaco com contexto, origem e cobertura observada da evidência.
    /// O chamador deve validar o payload e conferir o digest antes de reusar.
    pub payload_json: String,
}

/// Entry de escrita em lote. Os campos de contexto (mode/value/multipv) são
/// compartilhados por todas as entries do flush e vêm como parâmetros do chamador.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidencePut {
    /// Chave opaca que identifica o contexto da evidência.
    pub context_key: String,
    pub reached_depth: u32,
    pub cp: i32,
    pub lines_json: String,
    pub payload_json: String,
}

/// Vista sobre uma [`Connection`] para operações do cache de evidência
/// contextual. Short-lived: criada dentro do escopo de uma operação de banco.
pub struct EvidenceCache<'a> {
    conn: &'a Connection,
}

impl<'a> EvidenceCache<'a> {
    /// Cria uma vista sobre a conexão. A tabela é garantida pelas migrações de
    /// [`crate::db::open_file`] e `open_memory`.
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// SQL da consulta covering por modo; os parâmetros (key, value, multipv)
    /// são idênticos nos dois modos, bindados em ?1, ?2, ?3.
    fn lookup_sql(mode: Mode) -> &'static str {
        match mode {
            Mode::Time => {
                "SELECT cp, lines_json, reached_depth, payload_json FROM evidence_cache
                 WHERE context_key = ?1 AND source_mode = 'time' AND source_value >= ?2 AND multipv >= ?3
                 ORDER BY source_value ASC LIMIT 1"
            }
            Mode::Depth => {
                "SELECT cp, lines_json, reached_depth, payload_json FROM evidence_cache
                 WHERE context_key = ?1 AND reached_depth >= ?2 AND multipv >= ?3
                 ORDER BY reached_depth ASC LIMIT 1"
            }
        }
    }

    /// Executa a consulta covering num statement já preparado para o modo,
    /// devolvendo a primeira row (ou None).
    fn query_with_stmt(
        stmt: &mut rusqlite::Statement<'_>,
        key: &str,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Option<CachedEvidence>> {
        let mut rows = stmt.query(rusqlite::params![key, value, multipv])?;
        match rows.next()? {
            Some(row) => Ok(Some(CachedEvidence {
                cp: row.get(0)?,
                lines_json: row.get(1)?,
                reached_depth: row.get(2)?,
                payload_json: row.get(3)?,
            })),
            None => Ok(None),
        }
    }

    /// Consulta covering: um pedido de depth P é coberto por qualquer entry com
    /// `reached_depth >= P` (independente do source_mode); um pedido de time T
    /// só por entries de tempo com `source_value >= T`. Exige `multipv >=` e
    /// desempata pela entry mais rasa (menor overkill).
    pub fn lookup(
        &self,
        key: &str,
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Option<CachedEvidence>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
        Self::query_with_stmt(&mut stmt, key, value, multipv)
    }

    /// Consulta covering em lote, com um resultado por chave na ordem de
    /// entrada e o statement preparado uma única vez. Mesmas semânticas de
    /// [`EvidenceCache::lookup`].
    pub fn lookup_bulk(
        &self,
        keys: &[String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Vec<Option<CachedEvidence>>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
        keys.iter()
            .map(|key| Self::query_with_stmt(&mut stmt, key, value, multipv))
            .collect()
    }

    /// Grava uma entry. Mesma semântica de [`EvidenceCache::store_many`], para
    /// um único contexto.
    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &self,
        key: &str,
        mode: Mode,
        value: u32,
        multipv: u32,
        reached_depth: u32,
        cp: i32,
        lines_json: &str,
        payload_json: &str,
    ) -> rusqlite::Result<()> {
        self.conn.execute(
            INSERT_SQL,
            rusqlite::params![
                key,
                reached_depth,
                multipv,
                mode.as_str(),
                value,
                cp,
                lines_json,
                payload_json,
            ],
        )?;
        Ok(())
    }

    /// Grava N entries numa única transação atômica, com o statement preparado
    /// uma vez. Mesmo upsert condicional de [`EvidenceCache::store`].
    pub fn store_many(
        &self,
        entries: &[EvidencePut],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<()> {
        self.store_rows(entries.iter().map(|entry| (entry, multipv)), mode, value)
    }

    /// Grava N entries com cobertura MultiPV distinta numa única transação.
    pub fn store_many_covered(
        &self,
        entries: &[(EvidencePut, u32)],
        mode: Mode,
        value: u32,
    ) -> rusqlite::Result<()> {
        self.store_rows(
            entries.iter().map(|(entry, coverage)| (entry, *coverage)),
            mode,
            value,
        )
    }

    fn store_rows<'e>(
        &self,
        entries: impl IntoIterator<Item = (&'e EvidencePut, u32)>,
        mode: Mode,
        value: u32,
    ) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(INSERT_SQL)?;
            for (entry, multipv) in entries {
                stmt.execute(rusqlite::params![
                    entry.context_key,
                    entry.reached_depth,
                    multipv,
                    mode.as_str(),
                    value,
                    entry.cp,
                    entry.lines_json,
                    entry.payload_json,
                ])?;
            }
        }
        tx.commit()
    }

    /// Remove apenas as evidências contextuais, sem tocar no cache legado.
    pub fn clear(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM evidence_cache", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
