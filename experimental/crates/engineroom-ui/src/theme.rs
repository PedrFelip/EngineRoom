//! OpenCode-based color themes and EngineRoom typography.

use rune::{TextStyle, color::hex};
use rune_kit::Kit;
use std::sync::OnceLock;

/// Selects the light or dark EngineRoom appearance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Theme {
    /// The dark appearance.
    Dark,
    /// The light appearance.
    Light,
}

/// Font families used by the Rune application.
pub const FAMILIES: (&str, &str) = ("IBM Plex Sans", "IBM Plex Mono");

/// Embedded IBM Plex font files registered by the Rune application.
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../../../assets/IBMPlexSans-Regular.otf"),
    include_bytes!("../../../assets/IBMPlexSans-Medium.otf"),
    include_bytes!("../../../assets/IBMPlexSans-SemiBold.otf"),
    include_bytes!("../../../assets/IBMPlexMono-Regular.otf"),
    include_bytes!("../../../assets/IBMPlexMono-Medium.otf"),
];

/// Returns the cached Rune kit customized for the selected appearance.
pub fn kit(theme: Theme) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let (dark, slot) = match theme {
        Theme::Dark => (true, &DARK),
        Theme::Light => (false, &LIGHT),
    };
    slot.get_or_init(|| {
        let mut kit = rune_theme_opencode::kit(dark).clone();
        if dark {
            let accent = hex("#aa9b88").expect("valid accent color");
            kit.c.bg_accent = accent;
            kit.c.text_accent = accent;
            kit.c.icon_accent = accent;
            kit.c.border_focus = accent;
            kit.c.text_faint = hex("#999999").expect("valid faint text color");
        } else {
            kit.c.text_faint = hex("#696969").expect("valid faint text color");
            kit.c.border_focus = hex("#5878cf").expect("valid focus color");
        }
        kit.ty.ui = TextStyle::new(14.0, 400.0).lh(20.0);
        kit.ty.ui_medium = TextStyle::new(14.0, 500.0).lh(20.0);
        kit.ty.ui_compact = TextStyle::new(14.0, 400.0).lh(18.0);
        kit.ty.small = TextStyle::new(12.0, 400.0).lh(16.0);
        kit.ty.small_medium = TextStyle::new(12.0, 500.0).lh(16.0);
        kit.ty.body = kit.ty.ui;
        kit.ty.body_medium = kit.ty.ui_medium;
        kit.ty.text_small = TextStyle::new(13.0, 400.0).lh(18.0);
        kit.ty.text_small_medium = TextStyle::new(13.0, 500.0).lh(18.0);
        kit.ty.text_xsmall = kit.ty.small;
        kit.ty.text_xsmall_medium = kit.ty.small_medium;
        kit.ty.mono = TextStyle::new(13.0, 400.0).mono().lh(20.0);
        kit
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::Color;

    fn contrast(a: Color, b: Color) -> f32 {
        let (a, b) = (a.luminance(), b.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn secondary_text_and_focus_remain_distinguishable_on_common_surfaces() {
        for theme in [Theme::Dark, Theme::Light] {
            let colors = &kit(theme).c;
            for surface in [
                colors.bg_base,
                colors.bg_deep,
                colors.layer_01,
                colors.layer_02,
            ] {
                for text in [colors.text_muted, colors.text_faint] {
                    assert!(
                        contrast(text, surface) >= 4.5,
                        "text {} on {}: {:.2}",
                        text.hex(),
                        surface.hex(),
                        contrast(text, surface)
                    );
                }
                assert!(
                    contrast(colors.border_focus, surface) >= 3.0,
                    "focus {} on {}: {:.2}",
                    colors.border_focus.hex(),
                    surface.hex(),
                    contrast(colors.border_focus, surface)
                );
            }
        }
    }
}
