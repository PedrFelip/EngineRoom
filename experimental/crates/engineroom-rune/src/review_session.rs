//! Pure ordering and lifecycle state shared by native session event handling.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Phase {
    #[default]
    Idle,
    Opening,
    Running,
    Closing,
    Closed,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ReviewSession {
    pub phase: Phase,
    id: Option<String>,
    sequence: u64,
    request_id: u64,
}
impl ReviewSession {
    pub fn open(&mut self, id: String) {
        self.id = Some(id);
        self.sequence = 0;
        self.request_id = 0;
        self.phase = Phase::Opening;
    }
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }
    pub fn acquired(&mut self, id: &str) {
        if self.id() == Some(id) && self.phase == Phase::Opening {
            self.phase = Phase::Running;
        }
    }
    pub fn invalidate_live(&mut self) -> u64 {
        self.request_id = self
            .request_id
            .checked_add(1)
            .expect("live request ID exhausted");
        self.request_id
    }
    pub fn close(&mut self) -> Option<String> {
        if matches!(self.phase, Phase::Idle | Phase::Closed | Phase::Closing) {
            return None;
        }
        self.invalidate_live();
        self.phase = Phase::Closing;
        self.id.clone()
    }
    pub fn closed(&mut self, id: &str) {
        if self.id() == Some(id) {
            self.phase = Phase::Closed;
        }
    }
    pub fn accepts(&mut self, id: &str, sequence: u64, request_id: Option<u64>) -> bool {
        if self.id() != Some(id)
            || sequence <= self.sequence
            || matches!(self.phase, Phase::Idle | Phase::Closing | Phase::Closed)
        {
            return false;
        }
        self.sequence = sequence;
        request_id.is_none_or(|id| id == self.request_id)
    }
}
#[cfg(test)]
mod tests {
    use super::{Phase, ReviewSession};
    #[test]
    fn rejects_old_sessions_sequences_and_live_requests() {
        let mut s = ReviewSession::default();
        s.open("first".into());
        let old = s.invalidate_live();
        let latest = s.invalidate_live();
        assert!(!s.accepts("old", 1, Some(latest)));
        assert!(!s.accepts("first", 1, Some(old)));
        assert!(!s.accepts("first", 1, Some(latest)));
        assert!(s.accepts("first", 2, Some(latest)));
        s.open("replacement".into());
        s.closed("first");
        assert_eq!(s.phase, Phase::Opening);
        assert!(!s.accepts("first", 3, None));
        assert!(s.accepts("replacement", 1, None));
    }
    #[test]
    fn close_during_opening_requires_matching_ack() {
        let mut s = ReviewSession::default();
        s.open("first".into());
        assert_eq!(s.close().as_deref(), Some("first"));
        s.acquired("first");
        assert_eq!(s.phase, Phase::Closing);
        assert!(!s.accepts("first", 1, None));
        assert!(s.close().is_none());
        s.closed("other");
        assert_eq!(s.phase, Phase::Closing);
        s.closed("first");
        assert_eq!(s.phase, Phase::Closed);
    }
}
