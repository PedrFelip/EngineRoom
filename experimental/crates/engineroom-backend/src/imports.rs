//! Native Chess.com imports with bounded HTTP and owner-scoped tasks.
mod chess_com;
mod http;
mod tasks;

pub use chess_com::{
    Before, Cursor, ErrorCode, Http, ImportError, Limits, Player, RemoteGame, Result, SearchResult,
    StopReason, Task, search, username,
};
pub use http::ChessComHttp;
pub use tasks::{Event, EventBody, ImportTasks, SearchRequest};

#[cfg(test)]
mod tests;
