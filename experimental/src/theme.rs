//! Rune’s warm graphite palette and IBM Plex typography.
use crate::state::Appearance;
use rune::{Color, TextStyle, color::hex, oklch};
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
        // Retain the existing light appearance.
        if dark {
            k.c.bg_base = oklch(0.214, 0.009, 43.1);
            k.c.layer_01 = oklch(0.214, 0.009, 43.1);
            k.c.layer_02 = oklch(0.268, 0.011, 36.5);
            k.c.text_base = oklch(0.986, 0.002, 67.8);
            k.c.text_muted = oklch(0.714, 0.014, 41.2);
        } else {
            k.c.bg_base = oklch(1.0, 0.0, 0.0);
            k.c.layer_01 = oklch(1.0, 0.0, 0.0);
            k.c.layer_02 = oklch(0.96, 0.002, 17.2);
            k.c.text_base = oklch(0.147, 0.004, 49.3);
            k.c.text_muted = oklch(0.547, 0.021, 43.1);
        }
        // Shared control roles for the light appearance and semantic states.
        let (secondary, border, focus, primary, foreground, destructive) = if dark {
            (
                oklch(0.268, 0.011, 36.5),
                oklch(1.0, 0.0, 0.0).alpha(0.10),
                oklch(0.547, 0.021, 43.1),
                oklch(0.922, 0.005, 34.3),
                oklch(0.214, 0.009, 43.1),
                oklch(0.704, 0.191, 22.216),
            )
        } else {
            (
                oklch(0.96, 0.002, 17.2),
                oklch(0.922, 0.005, 34.3),
                oklch(0.714, 0.014, 41.2),
                oklch(0.214, 0.009, 43.1),
                oklch(0.986, 0.002, 67.8),
                oklch(0.577, 0.245, 27.325),
            )
        };
        k.c.layer_03 = secondary;
        k.c.bg_button_neutral = secondary;
        k.c.surface_base = k.c.bg_base;
        k.c.surface_weak = secondary;
        k.c.surface_raised = secondary;
        k.c.surface_raised_base = secondary;
        k.c.border_base = border;
        k.c.border_weak = border;
        k.c.border_focus = focus;
        k.c.bg_accent = primary;
        k.c.text_accent = primary;
        k.c.icon_accent = primary;
        k.c.submit_fg = foreground;
        k.c.submit_icon = foreground;
        k.c.danger.fg = destructive;
        k.c.danger.icon = destructive;
        if dark {
            let panel = hex("#171717").expect("valid panel color");
            let raised = hex("#242424").expect("valid raised color");
            let border = hex("#383838").expect("valid border color");
            let ink = hex("#f2f0eb").expect("valid text color");
            let muted = hex("#aaa7a1").expect("valid muted color");
            let accent = hex("#c5a880").expect("valid accent color");
            k.c.page = Color::BLACK;
            k.c.bg_deep = Color::BLACK;
            k.c.bg_base = panel;
            k.c.layer_01 = panel;
            k.c.layer_02 = raised;
            k.c.layer_03 = raised;
            k.c.surface_base = panel;
            k.c.surface_weak = raised;
            k.c.surface_raised = raised;
            k.c.surface_raised_base = raised;
            k.c.bg_button_neutral = raised;
            k.c.border_base = border;
            k.c.border_weak = border;
            k.c.border_strong = border;
            k.c.border_focus = accent;
            k.c.text_base = ink;
            k.c.text_muted = muted;
            k.c.icon_base = ink;
            k.c.icon_muted = muted;
            k.c.bg_accent = accent;
            k.c.text_accent = accent;
            k.c.icon_accent = accent;
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
