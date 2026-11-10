//! Matching loaded history entries without regard to case or accents.
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

fn normalize(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// A normalized query reusable across all entries in a history render.
pub(super) struct HistorySearch(String);

impl HistorySearch {
    pub(super) fn new(query: &str) -> Self {
        Self(normalize(query.trim()))
    }

    pub(super) fn matches(&self, white: &str, black: &str) -> bool {
        normalize(&format!("{white} {black}")).contains(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::HistorySearch;

    #[test]
    fn matches_either_player_case_and_accent_insensitively() {
        assert!(HistorySearch::new("  JOSE  ").matches("José Capablanca", "MÁRIA"));
        assert!(HistorySearch::new("maria").matches("José", "MÁRIA"));
        assert!(HistorySearch::new("josé").matches("Jose\u{301}", "Maria"));
        assert!(!HistorySearch::new("carlsen").matches("José", "Maria"));
        assert!(HistorySearch::new("  ").matches("José", "Maria"));
    }

    #[test]
    fn one_query_can_match_multiple_entries_and_player_boundary() {
        let search = HistorySearch::new("é ma");
        assert!(search.matches("José", "Maria"));
        assert!(search.matches("JOSE", "Magnus"));
        assert!(!search.matches("Maria", "José"));
    }
}
