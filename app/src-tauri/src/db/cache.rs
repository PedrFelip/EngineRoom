//! Cache de posições avaliadas pelo engine.
//!
//! O cache é unificado entre modos (depth/time), chaveado por
//! (fen, reached_depth, multipv). Novas entries do repository usam a menor
//! profundidade das linhas armazenadas; entries antigas podem usar a PV1 e
//! são revalidadas no adapter. A PK coalesce essas entries; isso não implica
//! determinismo da busca. `source_mode`/
//! `source_value` preservam o contexto da análise original: `source_mode` é
//! `"depth"` ou `"time"`; `source_value` é o ply pedido (depth) ou os
//! milissegundos (time).
//!
//! A consulta é covering: um pedido de depth P é servido por qualquer entry
//! com `reached_depth >= P`; um pedido de time T só é servido por entries de
//! time com `source_value >= T`. `lines_json` guarda as linhas candidatas no
//! formato do frontend (`[{multipv, cp, pv, san, depth}]`).
//!
//! A PK `(fen, reached_depth, multipv)` coalesce entries. Numa colisão, uma
//! entry de time com orçamento maior preserva sua proveniência contra uma
//! busca mais fraca (depth ou time menor), desde que o payload existente
//! continue consistente; rows corrompidas podem ser reparadas pela entry nova.

use crate::db::{mode::Mode, DbState};
use rusqlite::{Connection, OptionalExtension};

/// `INSERT OR REPLACE` compartilhado entre [`Cache::store`] e
/// [`Cache::store_many`] — a PK `(fen, reached_depth, multipv)` coalesce.
const INSERT_SQL: &str = "INSERT OR REPLACE INTO position_cache
    (fen, reached_depth, multipv, source_mode, source_value, cp, lines_json)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)";

/// Lê a entry que colidiria com a PK antes de decidir se ela pode ser trocada.
const EXISTING_SQL: &str = "SELECT source_mode, source_value, cp, lines_json
    FROM position_cache
    WHERE fen = ?1 AND reached_depth = ?2 AND multipv = ?3";

/// `true` quando uma colisão trocaria uma proveniência de time mais longa por
/// uma busca mais fraca: depth não carrega orçamento de tempo, e um time menor
/// cobre menos pedidos. Orçamentos iguais ou maiores podem atualizar a row.
fn weakens_timed_provenance(existing: &str, existing_value: i64, mode: Mode, value: u32) -> bool {
    match (existing, mode) {
        ("time", Mode::Depth) => true,
        ("time", Mode::Time) => existing_value > i64::from(value),
        _ => false,
    }
}

/// Only preserve a timed row that can serve its advertised coverage and depth.
fn payload_is_consistent(
    fen: &str,
    multipv: u32,
    reached_depth: u32,
    cp: i32,
    lines_json: &str,
) -> bool {
    let hit = CachedPosition {
        cp,
        lines_json: lines_json.to_owned(),
        reached_depth,
    };
    crate::review::repository::shape(hit, fen, multipv).is_some_and(|raw| {
        raw.lines
            .iter()
            .all(|line| line.depth.unwrap_or(reached_depth) >= reached_depth)
    })
}

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

    /// SQL das entries covering por modo; os parâmetros (fen, value, multipv)
    /// são idênticos nos dois modos, bindados em ?1, ?2, ?3. A ordem primária
    /// preserva a cobertura (entry mais rasa em depth; menor orçamento em
    /// time); os desempates só tornam a escolha determinística.
    fn lookup_sql(mode: Mode) -> &'static str {
        match mode {
            Mode::Time => {
                "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND source_mode = 'time' AND source_value >= ?2 AND multipv >= ?3
                 ORDER BY source_value ASC, reached_depth ASC, multipv ASC"
            }
            Mode::Depth => {
                "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND reached_depth >= ?2 AND multipv >= ?3
                 ORDER BY reached_depth ASC, multipv ASC, source_value ASC"
            }
        }
    }

    fn read_hit(row: &rusqlite::Row<'_>) -> rusqlite::Result<CachedPosition> {
        Ok(CachedPosition {
            cp: row.get(0)?,
            lines_json: row.get(1)?,
            reached_depth: row.get(2)?,
        })
    }

    /// Executa a consulta covering num statement já preparado para o modo,
    /// devolvendo a primeira row (ou None).
    fn query_with_stmt(
        stmt: &mut rusqlite::Statement<'_>,
        fen: &str,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Option<CachedPosition>> {
        let mut rows = stmt.query(rusqlite::params![fen, value, multipv])?;
        match rows.next()? {
            Some(row) => Ok(Some(Self::read_hit(row)?)),
            None => Ok(None),
        }
    }

    /// Todas as rows covering na ordem da consulta. O adapter percorre a lista
    /// até a primeira que sobrevive à validação de payload e profundidades;
    /// corrupção vira miss local, erro real do SQLite continua fatal.
    fn query_candidates_with_stmt(
        stmt: &mut rusqlite::Statement<'_>,
        fen: &str,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Vec<CachedPosition>> {
        let mut rows = stmt.query(rusqlite::params![fen, value, multipv])?;
        let mut hits = Vec::new();
        while let Some(row) = rows.next()? {
            hits.push(Self::read_hit(row)?);
        }
        Ok(hits)
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
    ) -> rusqlite::Result<Option<CachedPosition>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
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
    ) -> rusqlite::Result<Vec<Option<CachedPosition>>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
        fens.iter()
            .map(|fen| Self::query_with_stmt(&mut stmt, fen, value, multipv))
            .collect()
    }

    /// Todas as rows covering de uma posição, na ordem de [`Cache::lookup`].
    /// Usada pelo adapter para validar candidatos em sequência e só então
    /// escolher a primeira entry utilizável.
    #[cfg(test)]
    pub fn lookup_candidates(
        &self,
        fen: &str,
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Vec<CachedPosition>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
        Self::query_candidates_with_stmt(&mut stmt, fen, value, multipv)
    }

    /// Versão em lote de [`Cache::lookup_candidates`]: um `Vec` por fen, na
    /// mesma ordem de entrada e com o statement preparado uma única vez.
    pub fn lookup_bulk_candidates(
        &self,
        fens: &[String],
        mode: Mode,
        value: u32,
        multipv: u32,
    ) -> rusqlite::Result<Vec<Vec<CachedPosition>>> {
        let mut stmt = self.conn.prepare(Self::lookup_sql(mode))?;
        fens.iter()
            .map(|fen| Self::query_candidates_with_stmt(&mut stmt, fen, value, multipv))
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
    ) -> rusqlite::Result<()> {
        let entry = CachedPositionPut {
            fen: fen.to_owned(),
            reached_depth,
            cp,
            lines_json: lines_json.to_owned(),
        };
        self.store_rows(std::iter::once((&entry, multipv)), mode, value)
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
    ) -> rusqlite::Result<()> {
        self.store_rows(entries.iter().map(|entry| (entry, multipv)), mode, value)
    }

    /// One atomic flush even when searches produced different MultiPV coverage.
    pub(crate) fn store_many_covered(
        &self,
        entries: &[(CachedPositionPut, u32)],
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
        entries: impl IntoIterator<Item = (&'e CachedPositionPut, u32)>,
        mode: Mode,
        value: u32,
    ) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut existing = tx.prepare(EXISTING_SQL)?;
            let mut stmt = tx.prepare(INSERT_SQL)?;
            for (entry, multipv) in entries {
                let current = existing
                    .query_row(
                        rusqlite::params![entry.fen, entry.reached_depth, multipv],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, i64>(1)?,
                                row.get::<_, i32>(2)?,
                                row.get::<_, String>(3)?,
                            ))
                        },
                    )
                    .optional()?;
                if let Some((current_mode, current_value, cp, lines_json)) = current {
                    if weakens_timed_provenance(&current_mode, current_value, mode, value)
                        && payload_is_consistent(
                            &entry.fen,
                            multipv,
                            entry.reached_depth,
                            cp,
                            &lines_json,
                        )
                    {
                        continue;
                    }
                }
                stmt.execute(rusqlite::params![
                    entry.fen,
                    entry.reached_depth,
                    multipv,
                    mode,
                    value,
                    entry.cp,
                    entry.lines_json,
                ])?;
            }
        }
        tx.commit()
    }

    pub fn clear(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM position_cache", [])?;
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
    Cache::new(&conn)
        .lookup(fen, mode, depth, multipv)
        .map_err(|e| e.to_string())
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
    Cache::new(&conn)
        .store(fen, mode, depth, multipv, reached_depth, cp, lines_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cache_clear(state: tauri::State<'_, DbState>) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    Cache::new(&conn).clear().map_err(|e| e.to_string())
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
    Cache::new(&conn)
        .lookup_bulk(&fens, mode, depth, multipv)
        .map_err(|e| e.to_string())
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
    Cache::new(&conn)
        .store_many(&entries, mode, depth, multipv)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "cache/tests.rs"]
mod tests;
