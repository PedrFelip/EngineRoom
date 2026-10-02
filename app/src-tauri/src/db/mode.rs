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

impl ToSql for Mode {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::Borrowed(ValueRef::Text(
            self.as_str().as_bytes(),
        )))
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
mod tests;
