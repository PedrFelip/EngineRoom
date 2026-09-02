//! Cache de posições avaliadas pelo engine.
//!
//! O cache é unificado entre modos (depth/time), chaveado por
//! (fen, reached_depth, multipv). `reached_depth` é a profundidade real
//! atingida pela pv-1; entries que chegam ao mesmo reached_depth coalescem
//! (o engine é determinístico num dado reached_depth). `source_mode`/
//! `source_value` preservam o contexto da análise original: `source_mode` é
//! `"depth"` ou `"time"`; `source_value` é o ply pedido (depth) ou os
//! milissegundos (time).
//!
//! A consulta é covering: um pedido de depth P é servido por qualquer entry
//! com `reached_depth >= P`; um pedido de time T só é servido por entries de
//! time com `source_value >= T`. `lines_json` guarda as linhas candidatas no
//! formato do frontend (`[{multipv, cp, pv, san, depth}]`).

use crate::db::{mode::Mode, DbState};
use rusqlite::Connection;

/// `INSERT OR REPLACE` compartilhado entre [`Cache::store`] e
/// [`Cache::store_many`] — a PK `(fen, reached_depth, multipv)` coalesce.
const INSERT_SQL: &str = "INSERT OR REPLACE INTO position_cache
    (fen, reached_depth, multipv, source_mode, source_value, cp, lines_json)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

/// Avaliação cacheada de uma posição. A chave (fen, reached_depth, multipv) é
/// conhecida por quem consulta, então só o payload volta — incluindo
/// `reached_depth` para que o frontend reconstrua a profundidade real.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPosition {
    pub cp: i32,
    pub lines_json: String,
    pub reached_depth: u32,
}

/// Entry de escrita em lote recebida do frontend (`cache_put_many`). Os campos
/// de contexto (mode/value/multipv) são compartilhados por toda a partida e
/// vêm como parâmetros do comando, não duplicados por entry.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPositionPut {
    pub fen: String,
    pub reached_depth: u32,
    pub cp: i32,
    pub lines_json: String,
}

/// Vista sobre uma [`Connection`] para operações de cache de posições.
/// Short-lived: criada dentro do escopo trancado de um comando Tauri.
pub struct Cache<'a> {
    conn: &'a Connection,
}

impl<'a> Cache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// SQL da consulta covering por modo; os parâmetros (fen, value, multipv)
    /// são idênticos nos dois modos, bindados em ?1, ?2, ?3.
    fn lookup_sql(mode: Mode) -> &'static str {
        match mode {
            Mode::Time => {
                "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND source_mode = 'time' AND source_value >= ?2 AND multipv >= ?3
                 ORDER BY source_value ASC LIMIT 1"
            }
            Mode::Depth => {
                "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND reached_depth >= ?2 AND multipv >= ?3
                 ORDER BY reached_depth ASC LIMIT 1"
            }
        }
    }

    /// Executa a consulta covering num statement já preparado para o modo,
    /// devolvendo a primeira row (ou None).
    fn query_with_stmt(
        stmt: &mut rusqlite::Statement<'_>,
        fen: &str,
        value: u32,
        multipv: u32,
    ) -> Result<Option<CachedPosition>, String> {
        let mut rows = stmt
            .query(rusqlite::params![fen, value, multipv])
            .map_err(|e| e.to_string())?;
        match rows.next().map_err(|e| e.to_string())? {
            Some(row) => Ok(Some(CachedPosition {
                cp: row.get(0).map_err(|e| e.to_string())?,
                lines_json: row.get(1).map_err(|e| e.to_string())?,
                reached_depth: row.get(2).map_err(|e| e.to_string())?,
            })),
            None => Ok(None),
        }
    }

    /// Consulta covering: um pedido de depth P é coberto por qualquer entry com
    /// `reached_depth >= P` (independente do source_mode — entries de time
    /// também servem depth); um pedido de time T só é coberto por entries de
    /// time com `source_value >= T`. Em ambos os casos exige `multipv >=` e
    /// desempata pela entry mais rasa (menor overkill).
    pub fn lookup(
        &self,
        fen: &str,
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Result<Option<CachedPosition>, String> {
        let mut stmt = self
            .conn
            .prepare(Self::lookup_sql(mode))
            .map_err(|e| e.to_string())?;
        Self::query_with_stmt(&mut stmt, fen, value, multipv)
    }

    /// Consulta covering em lote: um resultado por fen, na mesma ordem de
    /// entrada, com o statement preparado uma única vez (mesmo padrão de
    /// [`Cache::store_many`]). Mesmas semânticas de [`Cache::lookup`].
    pub fn lookup_bulk(
        &self,
        fens: &[String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Result<Vec<Option<CachedPosition>>, String> {
        let mut stmt = self
            .conn
            .prepare(Self::lookup_sql(mode))
            .map_err(|e| e.to_string())?;
        fens
            .iter()
            .map(|fen| Self::query_with_stmt(&mut stmt, fen, value, multipv))
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &self,
        fen: &str,
        mode: Mode,
        value: u32,
        multipv: u32,
        reached_depth: u32,
        cp: i32,
        lines_json: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                INSERT_SQL,
                rusqlite::params![fen, reached_depth, multipv, mode, value, cp, lines_json],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Grava N entries numa única transação, com o statement preparado uma vez.
    /// Mesma semântica de [`Cache::store`] (`INSERT OR REPLACE` na PK
    /// `(fen, reached_depth, multipv)`), mas num commit só — bem menos fsync
    /// que N comandos separados, e atômico.
    pub fn store_many(
        &self,
        entries: &[CachedPositionPut],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> Result<(), String> {
        let tx = self
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        {
            let mut stmt = tx.prepare(INSERT_SQL).map_err(|e| e.to_string())?;
            for entry in entries {
                stmt.execute(rusqlite::params![
                    entry.fen,
                    entry.reached_depth,
                    multipv,
                    mode,
                    value,
                    entry.cp,
                    entry.lines_json,
                ])
                .map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())
    }

    pub fn clear(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM position_cache", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tauri::command]
pub fn cache_get(
    state: tauri::State<'_, DbState>,
    fen: &str,
    mode: Mode,
    depth: u32,
    multipv: u32,
) -> Result<Option<CachedPosition>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).lookup(fen, mode, depth, multipv)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn cache_put(
    state: tauri::State<'_, DbState>,
    fen: &str,
    mode: Mode,
    depth: u32,
    multipv: u32,
    reached_depth: u32,
    cp: i32,
    lines_json: &str,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).store(fen, mode, depth, multipv, reached_depth, cp, lines_json)
}

#[tauri::command]
pub fn cache_clear(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).clear()
}

/// Prefetch em lote: devolve um `Option<CachedPosition>` por fen, na mesma
/// ordem de entrada. Adquire o lock uma única vez para toda a partida.
#[tauri::command]
pub fn cache_get_bulk(
    state: tauri::State<'_, DbState>,
    fens: Vec<String>,
    mode: Mode,
    depth: u32,
    multipv: u32,
) -> Result<Vec<Option<CachedPosition>>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).lookup_bulk(&fens, mode, depth, multipv)
}

/// Descarga em lote: grava N entries numa única transação, num único lock.
#[tauri::command]
pub fn cache_put_many(
    state: tauri::State<'_, DbState>,
    entries: Vec<CachedPositionPut>,
    mode: Mode,
    depth: u32,
    multipv: u32,
) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).store_many(&entries, mode, depth, multipv)
}

#[cfg(test)]
#[path = "cache/tests.rs"]
mod tests;
