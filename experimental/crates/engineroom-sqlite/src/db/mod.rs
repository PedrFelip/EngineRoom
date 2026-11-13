pub mod cache;
pub mod games;
mod migrations;
pub mod mode;
pub mod stats;

pub fn open_file(path: &std::path::Path) -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    migrations::migrate(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_memory() -> rusqlite::Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open_in_memory()?;
    migrations::migrate(&conn)?;
    Ok(conn)
}
