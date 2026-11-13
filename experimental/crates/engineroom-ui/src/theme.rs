//! OpenCode-based color themes and EngineRoom typography.

use rune::{TextStyle, color::hex};
use rune_kit::Kit;

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

/// Builds the Rune kit customized for the selected appearance.
pub fn kit(theme: Theme) -> Kit {
    let dark = theme == Theme::Dark;
    let mut kit = rune_theme_opencode::kit(dark).clone();
    // Keep surfaces on one warm neutral ramp, with cards lifted above the shell.
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
            "#242421", "#191917", "#1f1f1d", "#30302c", "#393933", "#494940", "#eeede7", "#bdbbb2",
            "#aaa89f", "#dedbd0", "#41413a", "#a5a398",
        )
    } else {
        (
            "#ffffff", "#fafaf9", "#ffffff", "#f5f5f4", "#e7e5e4", "#d6d3d1", "#0c0a09", "#6b5f58",
            "#6b5f58", "#292524", "#d6d3d1", "#78716c",
        )
    };
    let color = |value| hex(value).expect("valid EngineRoom palette color");
    kit.c.page = color(if dark { layer_01 } else { base });
    kit.c.bg_base = color(base);
    kit.c.bg_deep = color(deep);
    kit.c.layer_01 = color(layer_01);
    kit.c.layer_02 = color(layer_02);
    kit.c.layer_03 = color(layer_03);
    kit.c.layer_04 = color(layer_04);
    kit.c.text_base = color(text);
    kit.c.text_muted = color(muted);
    kit.c.text_faint = color(faint);
    kit.c.icon_base = color(text);
    kit.c.icon_muted = color(muted);
    kit.c.icon_faint = color(faint);
    kit.c.bg_accent = color(accent);
    kit.c.text_accent = color(accent);
    kit.c.icon_accent = color(accent);
    kit.c.border_muted = color(border);
    kit.c.border_base = color(border);
    kit.c.border_strong = color(focus);
    kit.c.border_focus = color(focus);
    kit.c.bg_inverse = color(text);
    kit.c.bg_contrast = color(accent);
    kit.c.bg_button_neutral = color(layer_02);
    kit.c.bg_icon_button_contrast = color(accent);
    kit.c.submit_fg = color(base);
    kit.c.submit_icon = color(base);
    kit.c.knob_on = color(base);
    if dark {
        // The inherited white depth sheen overwhelms a dark switch thumb.
        kit.c.depth_top = rune::Color::TRANSPARENT;
        kit.c.depth_bottom = rune::Color::TRANSPARENT;
    }
    kit.c.text_inverse = color(base);
    kit.c.text_contrast = color(base);
    kit.c.icon_inverse = color(base);
    kit.c.icon_contrast = color(base);
    kit.c.surface_base = color(base);
    kit.c.surface_weak = color(deep);
    kit.c.surface_hover = color(layer_02);
    kit.c.surface_raised = color(layer_01);
    kit.c.surface_raised_base = color(layer_01);
    kit.c.surface_raised_hover = color(layer_02);
    kit.c.surface_float = color(layer_01);
    kit.c.surface_disabled = color(layer_02);
    kit.c.border_weak = color(border);
    kit.c.border_hover = color(focus);
    kit.c.border_active = color(focus);
    kit.c.border_disabled = color(border);
    kit.c.text_strong = color(text);
    kit.c.text_weak = color(muted);
    kit.c.text_invert_strong = color(base);
    kit.c.icon_weaker = color(faint);
    kit.c.std_border = color(border);
    kit.c.std_icon = color(muted);
    kit.c.std_icon_invert = color(base);
    kit.c.std_icon_strong = color(text);
    kit.c.std_text = color(text);
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
        for appearance in [Theme::Dark, Theme::Light] {
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
