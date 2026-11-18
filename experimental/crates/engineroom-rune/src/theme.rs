//! Adapts the app's appearance setting to the shared Rune theme crate.

use crate::state::Appearance;

pub use engineroom_ui::{FAMILIES, FONTS, Theme, session_swatch};

pub fn kit_theme(k: &rune_kit::Kit) -> Theme {
    if k.dark { Theme::Dark } else { Theme::Light }
}

pub fn kit(appearance: Appearance) -> rune_kit::Kit {
    let theme = match appearance {
        Appearance::Dark => Theme::Dark,
        Appearance::Light => Theme::Light,
    };
    engineroom_ui::kit(theme)
}
