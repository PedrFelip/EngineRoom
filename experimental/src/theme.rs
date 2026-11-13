//! EngineRoom's warm neutral palette with IBM Plex typography throughout the component kit.
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
        // Match the main app's warm stone ramp while retaining Rune's component styling.
        let (
            base,
            deep,
            layer_01,
            layer_02,
            layer_03,
            layer_04,
            text,
            muted,
            faint,
            accent,
            border,
            focus,
        ) = if dark {
            (
                "#0c0a09", "#0c0a09", "#1c1917", "#292524", "#35302d", "#44403c", "#fafaf9",
                "#b5aaa3", "#a89d96", "#e7e5e4", "#44403c", "#a89d96",
            )
        } else {
            (
                "#ffffff", "#fafaf9", "#ffffff", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#0c0a09",
                "#6b5f58", "#6b5f58", "#292524", "#d6d3d1", "#78716c",
            )
        };
        let color = |value| hex(value).expect("valid EngineRoom palette color");
        k.c.page = color(if dark { layer_01 } else { base });
        k.c.bg_base = color(base);
        k.c.bg_deep = color(deep);
        k.c.layer_01 = color(layer_01);
        k.c.layer_02 = color(layer_02);
        k.c.layer_03 = color(layer_03);
        k.c.layer_04 = color(layer_04);
        k.c.text_base = color(text);
        k.c.text_muted = color(muted);
        k.c.text_faint = color(faint);
        k.c.icon_base = color(text);
        k.c.icon_muted = color(muted);
        k.c.icon_faint = color(faint);
        k.c.bg_accent = color(accent);
        k.c.text_accent = color(accent);
        k.c.icon_accent = color(accent);
        k.c.border_muted = color(border);
        k.c.border_base = color(border);
        k.c.border_strong = color(focus);
        k.c.border_focus = color(focus);
        k.c.bg_inverse = color(text);
        k.c.bg_contrast = color(accent);
        k.c.bg_button_neutral = color(layer_02);
        k.c.bg_icon_button_contrast = color(accent);
        k.c.submit_fg = color(base);
        k.c.submit_icon = color(base);
        k.c.knob_on = color(base);
        k.c.text_inverse = color(base);
        k.c.text_contrast = color(base);
        k.c.icon_inverse = color(base);
        k.c.icon_contrast = color(base);
        k.c.surface_base = color(base);
        k.c.surface_weak = color(deep);
        k.c.surface_hover = color(layer_02);
        k.c.surface_raised = color(layer_01);
        k.c.surface_raised_base = color(layer_01);
        k.c.surface_raised_hover = color(layer_02);
        k.c.surface_float = color(layer_01);
        k.c.surface_disabled = color(layer_02);
        k.c.border_weak = color(border);
        k.c.border_hover = color(focus);
        k.c.border_active = color(focus);
        k.c.border_disabled = color(border);
        k.c.text_strong = color(text);
        k.c.text_weak = color(muted);
        k.c.text_invert_strong = color(base);
        k.c.icon_weaker = color(faint);
        k.c.std_border = color(border);
        k.c.std_icon = color(muted);
        k.c.std_icon_invert = color(base);
        k.c.std_icon_strong = color(text);
        k.c.std_text = color(text);
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
    fn enabled_switch_thumb_is_distinguishable_from_track() {
        for appearance in [Appearance::Dark, Appearance::Light] {
            let colors = &kit(appearance).c;
            assert!(
                contrast(colors.knob_on, colors.bg_accent) >= 3.0,
                "switch thumb {} on {}: {:.2}",
                colors.knob_on.hex(),
                colors.bg_accent.hex(),
                contrast(colors.knob_on, colors.bg_accent)
            );
        }
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
