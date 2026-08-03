//! Tipo type-safe para o modo de análise: `Depth` (`go depth N`) ou `Time`
//! (`go movetime N`).
//!
//! Substitui as strings `"depth"`/`"time"` que circulavam entre frontend,
//! comandos Tauri e SQL. Antes, um typo como `"timer"` caía silenciosamente no
//! branch de depth em `cache_lookup`; agora a desserialização rejeita na
//! fronteira do comando. O wire format é preservado (`"depth"`/`"time"`
//! lowercase), então nenhum caller do frontend muda.

use rusqlite::types::{FromSql, FromSqlError, ToSql, ToSqlOutput, ValueRef};

/// Modo de análise. Serializa como `"depth"`/`"time"` (lowercase) no wire JSON
/// do Tauri e como TEXT no SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Depth,
    Time,
}

impl Mode {
    /// String canônica usada nas colunas TEXT do SQLite (`"depth"`/`"time"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Depth => "depth",
            Mode::Time => "time",
        }
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ToSql for Mode {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(self.as_str().as_bytes())))
    }
}

impl FromSql for Mode {
    fn column_result(value: ValueRef<'_>) -> Result<Self, FromSqlError> {
        match value.as_str()? {
            "depth" => Ok(Mode::Depth),
            "time" => Ok(Mode::Time),
            other => Err(FromSqlError::Other(
                format!("modo inválido: {other:?}").into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_serializa_lowercase_no_wire() {
        assert_eq!(serde_json::to_string(&Mode::Depth).unwrap(), "\"depth\"");
        assert_eq!(serde_json::to_string(&Mode::Time).unwrap(), "\"time\"");
    }

    #[test]
    fn mode_desserializa_de_strings_lowercase() {
        assert_eq!(
            serde_json::from_str::<Mode>("\"depth\"").unwrap(),
            Mode::Depth
        );
        assert_eq!(
            serde_json::from_str::<Mode>("\"time\"").unwrap(),
            Mode::Time
        );
    }

    #[test]
    fn mode_rejeita_string_invalida_no_wire() {
        assert!(serde_json::from_str::<Mode>("\"timer\"").is_err());
        // Case-sensitive: uppercase é rejeitado (wire é lowercase).
        assert!(serde_json::from_str::<Mode>("\"DEPTH\"").is_err());
        assert!(serde_json::from_str::<Mode>("\"\"").is_err());
    }

    #[test]
    fn mode_roundtrip_pelo_sqlite() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE m (mode TEXT NOT NULL);")
            .unwrap();
        conn.execute("INSERT INTO m (mode) VALUES (?1)", [Mode::Time])
            .unwrap();

        let lido: Mode = conn
            .query_row("SELECT mode FROM m", [], |r| r.get(0))
            .unwrap();
        assert_eq!(lido, Mode::Time);
    }

    #[test]
    fn from_sql_rejeita_texto_invalido() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE m (mode TEXT);
             INSERT INTO m (mode) VALUES ('timer');",
        )
        .unwrap();

        let res = conn.query_row("SELECT mode FROM m", [], |r| r.get::<_, Mode>(0));
        assert!(res.is_err(), "texto inválido deve falhar ao desserializar");
    }
}
