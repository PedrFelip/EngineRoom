//! Adapts the app's appearance setting to the shared Rune theme crate.

use crate::state::Appearance;

pub use engineroom_ui::{FAMILIES, FONTS};

pub fn kit(appearance: Appearance) -> rune_kit::Kit {
    let theme = match appearance {
        Appearance::Dark => engineroom_ui::Theme::Dark,
        Appearance::Light => engineroom_ui::Theme::Light,
    };
    engineroom_ui::kit(theme)
}
