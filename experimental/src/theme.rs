//! OpenCode's original palette with IBM Plex typography throughout the component kit.
use crate::state::Appearance;
use rune::TextStyle;
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
