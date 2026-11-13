pub use engineroom_domain::db::mode::Mode;

pub(crate) struct SqlMode(pub Mode);

impl rusqlite::types::FromSql for SqlMode {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_str()? {
            "depth" => Ok(Self(Mode::Depth)),
            "time" => Ok(Self(Mode::Time)),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("modo inválido: {other:?}").into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests;
