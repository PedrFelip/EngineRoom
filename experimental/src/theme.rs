//! OpenCode's original palette with IBM Plex typography throughout the component kit.
use crate::state::Appearance;
use rune::{TextStyle, color::hex};
use rune_kit::Kit;
use std::sync::OnceLock;

pub const FAMILIES: (&str, &str) = ("IBM Plex Sans", "IBM Plex Mono");
pub const FONTS: &[&[u8]] = &[
    include_bytes!("../assets/IBMPlexSans-Regular.otf"),
    include_bytes!("../assets/IBMPlexSans-Medium.otf"),
    include_bytes!("../assets/IBMPlexSans-SemiBold.otf"),
    include_bytes!("../assets/IBMPlexMono-Regular.otf"),
    include_bytes!("../assets/IBMPlexMono-Medium.otf"),
];
pub fn kit(appearance: Appearance) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    let dark = appearance == Appearance::Dark;
    let slot = if dark { &DARK } else { &LIGHT };
    slot.get_or_init(|| {
        let mut k = rune_theme_opencode::kit(dark).clone();
        if dark {
            let accent = hex("#aa9b88").expect("valid accent color");
            k.c.bg_accent = accent;
            k.c.text_accent = accent;
            k.c.icon_accent = accent;
            k.c.border_focus = accent;
            k.c.text_faint = hex("#999999").expect("valid faint text color");
        } else {
            k.c.text_faint = hex("#696969").expect("valid faint text color");
            k.c.border_focus = hex("#5878cf").expect("valid focus color");
        }
        k.ty.ui = TextStyle::new(14.0, 400.0).lh(20.0);
        k.ty.ui_medium = TextStyle::new(14.0, 500.0).lh(20.0);
        k.ty.ui_compact = TextStyle::new(14.0, 400.0).lh(18.0);
        k.ty.small = TextStyle::new(12.0, 400.0).lh(16.0);
        k.ty.small_medium = TextStyle::new(12.0, 500.0).lh(16.0);
        k.ty.body = k.ty.ui;
        k.ty.body_medium = k.ty.ui_medium;
        k.ty.text_small = TextStyle::new(13.0, 400.0).lh(18.0);
        k.ty.text_small_medium = TextStyle::new(13.0, 500.0).lh(18.0);
        k.ty.text_xsmall = k.ty.small;
        k.ty.text_xsmall_medium = k.ty.small_medium;
        k.ty.mono = TextStyle::new(13.0, 400.0).mono().lh(20.0);
        k
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
        for appearance in [Appearance::Dark, Appearance::Light] {
            let colors = &kit(appearance).c;
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
