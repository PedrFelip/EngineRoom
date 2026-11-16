//! Deferred native initialization, separate from the first rendered frame.
use crate::{bridge::Bridge, session_store::Store};
use std::sync::mpsc;

#[derive(Default)]
pub(crate) struct Startup {
    pub wake: Option<rune::Waker>,
    pub store: Option<Store>,
    pub loaded: bool,
    pub first_poll: bool,
    pub backend: Option<mpsc::Receiver<Result<Bridge, String>>>,
    pub backend_started: bool,
    pub error: Option<String>,
    pub last_checkpoint: f64,
    pub last_signature: Option<u64>,
    pub ready_reported: bool,
    pub avatar_key: Option<String>,
}
impl Startup {
    pub fn enabled(&self) -> bool {
        self.wake.is_some()
    }
    pub fn loading(&self) -> bool {
        self.enabled() && (!self.loaded || !self.ready_reported)
    }
}
