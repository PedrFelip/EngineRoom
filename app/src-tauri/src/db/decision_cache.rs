//! Versioned contextual evidence cache, isolated from legacy FEN-only entries.
use crate::review::decision::{satisfies, Evidence, Request};
use rusqlite::{params, Connection};

pub struct DecisionCache<'a>(pub &'a Connection);

impl DecisionCache<'_> {
    /// Corrupt or incompatible payloads are misses; SQLite failures propagate.
    pub fn lookup(&self, requests: &[Request]) -> rusqlite::Result<Vec<Option<Evidence>>> {
        let mut stmt = self.0.prepare_cached(
            "SELECT evidence_json FROM decision_cache_v1 WHERE context_json = ?1 AND engine_json = ?2 AND depth >= ?3 AND multipv >= ?4 ORDER BY depth, multipv",
        )?;
        requests
            .iter()
            .map(|request| {
                let context = serde_json::to_string(&request.context).map_err(json_error)?;
                let engine = serde_json::to_string(&request.engine).map_err(json_error)?;
                let rows = stmt.query_map(
                    params![context, engine, request.depth, request.multipv as i64],
                    |row| row.get::<_, String>(0),
                )?;
                for row in rows {
                    let payload = row?;
                    if let Ok(evidence) = serde_json::from_str::<Evidence>(&payload) {
                        if satisfies(&evidence, request) {
                            return Ok(Some(evidence));
                        }
                    }
                }
                Ok(None)
            })
            .collect()
    }

    /// Only complete valid evidence can enter the cache. Writes are atomic per batch.
    pub fn put(&self, entries: &[(Request, Evidence)]) -> rusqlite::Result<()> {
        let tx = self.0.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare_cached("INSERT OR REPLACE INTO decision_cache_v1 (context_json, engine_json, depth, multipv, evidence_json) VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for (request, evidence) in entries {
                if !satisfies(evidence, request) {
                    continue;
                }
                stmt.execute(params![
                    serde_json::to_string(&request.context).map_err(json_error)?,
                    serde_json::to_string(&request.engine).map_err(json_error)?,
                    evidence.depth,
                    request.multipv as i64,
                    serde_json::to_string(evidence).map_err(json_error)?
                ])?;
            }
        }
        tx.commit()
    }
}
fn json_error(error: serde_json::Error) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::decision::{self, Bound, EngineSignature, Line, Score};

    #[test]
    fn contextual_round_trip_and_corruption_are_isolated() {
        let conn = crate::db::open_memory().unwrap();
        let cache = DecisionCache(&conn);
        let context = decision::decisions("1. e4 *").unwrap().remove(0).before;
        let request = Request {
            context,
            engine: EngineSignature {
                executable_digest: "a".repeat(64),
                options: vec![("MultiPV".into(), "1".into())],
            },
            depth: 8,
            multipv: 1,
        };
        let evidence = Evidence {
            id: 1,
            execution_id: 1,
            context: request.context.clone(),
            engine: request.engine.clone(),
            unrestricted: true,
            depth: 8,
            lines: vec![Line {
                depth: 8,
                execution_id: 1,
                score: Score::Cp(20),
                bound: Bound::Exact,
                pv: vec!["e2e4".into()],
            }],
        };
        cache.put(&[(request.clone(), evidence.clone())]).unwrap();
        assert_eq!(
            cache.lookup(std::slice::from_ref(&request)).unwrap(),
            vec![Some(evidence)]
        );
        let mut other = request.clone();
        other.context.history = decision::History::BeforeRootUnknown;
        assert_eq!(cache.lookup(&[other]).unwrap(), vec![None]);
        conn.execute("UPDATE decision_cache_v1 SET evidence_json = 'broken'", [])
            .unwrap();
        assert_eq!(cache.lookup(&[request]).unwrap(), vec![None]);
        crate::db::migrations::migrate(&conn).unwrap();
    }
}
