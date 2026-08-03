//! Persistência SQLite: cache de posições avaliadas pelo engine ([`cache`]) e
//! store de partidas revisadas ([`games`]). Migrações em [`migrations`] rodam a
//! cada startup; estatísticas de armazenamento em [`stats`].
//!
//! Arquitetura: cada submódulo expõe um core puro sobre `&Connection`
//! ([`Cache`], [`Store`], [`Stats`]) que é o seam de testes; os comandos Tauri
//! (`cache_get`, `games_save`, ...) são adapters finos que lockam o mutex e
//! delegam. O [`mode::Mode`] tipa o modo de análise no wire e no SQL.

pub mod mode;

pub(crate) mod cache;
pub(crate) mod games;
mod migrations;
pub(crate) mod stats;

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

/// Conexão SQLite compartilhada pelos comandos Tauri.
pub struct DbState(pub Mutex<Connection>);

/// Abre (criando, se preciso) o banco em disco e aplica as migrações. O
/// diretório pai deve existir.
pub fn open_file(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    migrations::migrate(&conn)?;
    Ok(conn)
}

/// Abre um banco in-memory já migrado (apenas em testes).
#[cfg(test)]
pub fn open_memory() -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    migrations::migrate(&conn)?;
    Ok(conn)
}
