//! Compatibility exports for shared EngineRoom UI primitives.

pub use engineroom_ui::{
    button, button_icon, button_trailing, group, heading, integer_slider, label, mono, page, rule,
    section, setting, slider,
};

/// Analysis status with a soft diagonal highlight moving across the glyphs.
pub fn analysis_label(
    value: &str,
    font: rune::TextStyle,
    s: &crate::state::State,
    k: &rune_kit::Kit,
) -> rune::El {
    use rune::{Easing, LoopSpec, el, text};
    let mut label = el().child(text(value).font(font).color(k.c.text_muted));
    if s.preferences.reduced_motion {
        return label;
    }
    let (width, height) = rune::text::measure(value, &[], font, None);
    let period = 2.6;
    let delay = (-s.clock).rem_euclid(period);
    let delay = if delay > period - 0.001 { 0.0 } else { delay };
    let band = 32.0;
    for strip in 0..(height / 2.0).ceil() as usize {
        let y = strip as f32 * 2.0;
        let x0 = -band - y * 0.65;
        let x1 = width + band - y * 0.65;
        let slide = |from, to| LoopSpec {
            delay,
            ..LoopSpec::slide(period, [from, 0.0], [to, 0.0]).easing(Easing::LINEAR)
        };
        let bright = el()
            .w(band)
            .h(height)
            .looping(slide(-x0 / band, -x1 / band))
            .child(
                text(value)
                    .font(font)
                    .color(k.c.text_base)
                    .absolute(0.0, -y),
            );
        let sweep = el()
            .w(band)
            .h(2.0)
            .absolute(0.0, 0.0)
            .clip_fade(band / 2.0, 0.0, band / 2.0, 0.0)
            .looping(slide(x0 / band, x1 / band))
            .child(bright);
        label = label.child(
            el().key(format!("glare-strip-{strip}"))
                .absolute(0.0, y)
                .w(width)
                .h(2.0)
                .clip_rounded(0.0)
                .child(sweep)
                .inert()
                .a11y_hidden(),
        );
    }
    label
}

/// Adds consistent tactile feedback without changing an icon button's hit area.
pub fn icon_feedback(button: rune::El, k: &rune_kit::Kit) -> rune::El {
    button
        .press_scale(0.92)
        .motion(k.motion.control)
        .hover_motion(k.motion.control, k.motion.control)
}
