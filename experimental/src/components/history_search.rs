//! Matching loaded history entries without regard to case or accents.
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

fn normalize(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn matches(white: &str, black: &str, query: &str) -> bool {
    let query = normalize(query.trim());
    normalize(&format!("{white} {black}")).contains(&query)
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn matches_either_player_case_and_accent_insensitively() {
        assert!(matches("José Capablanca", "MÁRIA", "  JOSE  "));
        assert!(matches("José", "MÁRIA", "maria"));
        assert!(matches("Jose\u{301}", "Maria", "josé"));
        assert!(!matches("José", "Maria", "carlsen"));
        assert!(matches("José", "Maria", "  "));
    }
}
