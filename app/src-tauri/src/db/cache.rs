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

/// Vista sobre uma [`Connection`] para operações de cache de posições.
/// Short-lived: criada dentro do escopo trancado de um comando Tauri.
pub struct Cache<'a> {
    conn: &'a Connection,
}

impl<'a> Cache<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
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
        // Os dois modos diferem só no SQL — os parâmetros (fen, value, multipv)
        // são idênticos, bindados em ?1, ?2, ?3.
        let sql = match mode {
            Mode::Time => "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND source_mode = 'time' AND source_value >= ?2 AND multipv >= ?3
                 ORDER BY source_value ASC LIMIT 1",
            Mode::Depth => "SELECT cp, lines_json, reached_depth FROM position_cache
                 WHERE fen = ?1 AND reached_depth >= ?2 AND multipv >= ?3
                 ORDER BY reached_depth ASC LIMIT 1",
        };
        let mut stmt = self.conn.prepare(sql).map_err(|e| e.to_string())?;
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
        self.conn.execute(
            "INSERT OR REPLACE INTO position_cache
                (fen, reached_depth, multipv, source_mode, source_value, cp, lines_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![fen, reached_depth, multipv, mode, value, cp, lines_json],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_memory;

    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    const LINES: &str = r#"[{"multipv":1,"cp":35,"pv":["e2e4","e7e5"],"san":"e4"}]"#;

    #[test]
    fn cache_put_depois_get_devolve_posicao_armazenada() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

        let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap();

        assert_eq!(
            hit,
            Some(CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
                reached_depth: 20,
            })
        );
    }

    #[test]
    fn cache_armazena_reached_distinto_do_source_value_em_mode_time() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        // Modo tempo: source_value (movetimeMs) = 5000, reached_depth (plies) = 28.
        cache.store(FEN, Mode::Time, 5000, 1, 28, 35, LINES).unwrap();

        let hit = cache.lookup(FEN, Mode::Time, 5000, 1).unwrap();

        assert_eq!(
            hit,
            Some(CachedPosition {
                cp: 35,
                lines_json: LINES.to_string(),
                reached_depth: 28,
            })
        );
    }

    #[test]
    fn cache_get_em_posicao_desconhecida_devolve_none() {
        let conn = open_memory().unwrap();

        assert_eq!(
            Cache::new(&conn).lookup(FEN, Mode::Depth, 20, 1).unwrap(),
            None
        );
    }

    #[test]
    fn cache_cobre_depth_menor_com_entry_mais_profunda() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        // Posição analisada até reached_depth=20.
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

        // Pedido de depth 15 → deve acertar (20 >= 15).
        let hit = cache.lookup(FEN, Mode::Depth, 15, 1).unwrap();
        assert!(hit.is_some(), "depth menor deve ser coberto pela entry mais profunda");
        assert_eq!(hit.unwrap().cp, 35);
    }

    #[test]
    fn cache_nao_cobre_depth_maior_que_o_reached() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

        // Pedido de depth 25 → miss (20 < 25).
        assert_eq!(cache.lookup(FEN, Mode::Depth, 25, 1).unwrap(), None);
    }

    #[test]
    fn cache_depth_coberto_por_entry_de_time_mode_cross_mode() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        // Entry de time: reached_depth=28 (source_value/movetime não importa
        // para cobrir depth, só reached_depth conta).
        cache.store(FEN, Mode::Time, 5000, 1, 28, 35, LINES).unwrap();

        // Pedido de depth 20 → hit: 28 >= 20, independente do source_mode.
        let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap();
        assert!(hit.is_some(), "entry de time cobre depth");
        assert_eq!(hit.unwrap().reached_depth, 28);
    }

    #[test]
    fn cache_desempata_pela_entry_mais_rasa_em_depth() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 20, LINES).unwrap();
        cache.store(FEN, Mode::Depth, 30, 1, 30, 99, "[]").unwrap();

        // Pedido de depth 15 → ambas cobrem; deve devolver a mais rasa (20).
        let hit = cache.lookup(FEN, Mode::Depth, 15, 1).unwrap().unwrap();
        assert_eq!(hit.reached_depth, 20);
        assert_eq!(hit.cp, 20);
    }

    #[test]
    fn cache_cobre_multipv_menor_com_entry_de_mais_linhas() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        let lines4 = r#"[
            {"multipv":1,"cp":35,"pv":["e2e4"],"san":"e4"},
            {"multipv":2,"cp":30,"pv":["d2d4"],"san":"d4"},
            {"multipv":3,"cp":28,"pv":["c2c4"],"san":"c4"},
            {"multipv":4,"cp":25,"pv":["g1f3"],"san":"Nf3"}
        ]"#;
        cache.store(FEN, Mode::Depth, 20, 4, 20, 35, lines4).unwrap();

        // Pedido multipv=2 → coberto pela entry multipv=4.
        let hit = cache.lookup(FEN, Mode::Depth, 20, 2).unwrap();
        assert!(hit.is_some(), "entry com mais multipv cobre pedido menor");
    }

    #[test]
    fn cache_nao_cobre_multipv_maior_que_o_armazenado() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();

        // Pedido multipv=3 com entry multipv=1 → miss.
        assert_eq!(cache.lookup(FEN, Mode::Depth, 20, 3).unwrap(), None);
    }

    #[test]
    fn cache_time_coberto_por_entry_com_movetime_maior() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Time, 5000, 1, 28, 35, LINES).unwrap();

        // Pedido de time 3000ms → hit: 5000 >= 3000.
        let hit = cache.lookup(FEN, Mode::Time, 3000, 1).unwrap();
        assert!(hit.is_some(), "movetime maior cobre pedido menor");
        assert_eq!(hit.unwrap().reached_depth, 28);
    }

    #[test]
    fn cache_time_nao_coberto_por_entry_de_depth_mode() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        // Entry de depth: reached_depth=28 mas sem orçamento temporal.
        cache.store(FEN, Mode::Depth, 28, 1, 28, 35, LINES).unwrap();

        // Pedido de time → miss: depth entries não têm source_mode='time'.
        assert_eq!(cache.lookup(FEN, Mode::Time, 1000, 1).unwrap(), None);
    }

    #[test]
    fn cache_coalescer_entries_de_modos_diferentes_no_mesmo_reached() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        // Depth entry com reached=28.
        cache.store(FEN, Mode::Depth, 28, 1, 28, 35, LINES).unwrap();
        // Time entry no mesmo reached=28 → coalesce (PK é fen+reached+multipv),
        // último write vence.
        cache.store(FEN, Mode::Time, 5000, 1, 28, 42, "[]").unwrap();

        // Consulta de depth 20 acha a entry coalescida.
        let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().unwrap();
        assert_eq!(hit.cp, 42, "último write vence no coalesce");

        // Exatamente 1 row para esta FEN (não duas, uma por modo).
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM position_cache WHERE fen = ?1",
                [FEN],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "entries dos dois modos coalesceram num único row");
    }

    #[test]
    fn cache_put_repetido_sobrescreve_entrada() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();
        cache.store(FEN, Mode::Depth, 20, 1, 20, 42, "[]").unwrap();

        let hit = cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().unwrap();

        assert_eq!(hit.cp, 42);
        assert_eq!(hit.lines_json, "[]");
    }

    #[test]
    fn clear_cache_esvazia_tabela_de_posicoes() {
        let conn = open_memory().unwrap();
        let cache = Cache::new(&conn);
        cache.store(FEN, Mode::Depth, 20, 1, 20, 35, LINES).unwrap();
        cache.store(FEN, Mode::Time, 5000, 1, 28, 35, LINES).unwrap();
        assert!(
            cache.lookup(FEN, Mode::Depth, 20, 1).unwrap().is_some(),
            "pré-condição: cache populado"
        );

        cache.clear().unwrap();

        assert_eq!(
            cache.lookup(FEN, Mode::Depth, 20, 1).unwrap(),
            None,
            "entrada depth removida"
        );
        assert_eq!(
            cache.lookup(FEN, Mode::Time, 5000, 1).unwrap(),
            None,
            "entrada time removida"
        );
    }
}
