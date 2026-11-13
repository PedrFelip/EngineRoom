//! Reusable, application-state-generic Rune controls and layout primitives.

use rune::{Color, Drag, El, Icon, Phase, ScrollSpec, TextStyle, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit};

/// Creates a label with a minimum readable font size.
pub fn label(value: impl Into<String>, size: f32, color: Color) -> El {
    text(value)
        .font(TextStyle::new(size.max(12.0), 400.0))
        .color(color)
}

/// Creates a page heading using the supplied theme.
pub fn heading(value: &str, kit: &Kit) -> El {
    text(value)
        .font(TextStyle::new(24.0, 600.0))
        .color(kit.c.text_base)
        .wrap()
        .min_w(0.0)
}

/// Creates a one-point separator line.
pub fn rule(kit: &Kit) -> El {
    el().h(1.0).w_full().fill(kit.c.border_base).a11y_hidden()
}

/// Creates a prominent button with a stable element key.
pub fn button(key: &str, title: &str, kit: &Kit) -> El {
    Button::new(title)
        .control_size(ControlSize::ExtraLarge)
        .build(kit)
        .key(key)
}

/// Creates a prominent icon button with a stable element key.
pub fn button_icon(key: &str, title: &str, icon: &'static Icon, kit: &Kit) -> El {
    Button::new(title)
        .icon(icon)
        .control_size(ControlSize::ExtraLarge)
        .build(kit)
        .key(key)
}

/// Creates a button whose label includes a right chevron.
pub fn button_trailing(key: &str, title: &str, variant: ButtonVariant, kit: &Kit) -> El {
    let label = title.to_owned();
    Button::new(title)
        .variant(variant)
        .control_size(ControlSize::ExtraLarge)
        .label_with(move |font, ink| {
            row()
                .gap(6.0)
                .center_y()
                .child(text(label).font(font).color(ink))
                .child(icon(&super::icons::RIGHT, 16.0).color(ink))
        })
        .build(kit)
        .key(key)
}

/// Wraps content in the standard bordered surface.
pub fn group(content: El, kit: &Kit) -> El {
    col()
        .w_full()
        .padding(16.0)
        .radius(12.0)
        .border(1.0, kit.c.border_base)
        .fill(kit.c.bg_base)
        .child(content)
}

/// Creates a titled section with the standard bordered content surface.
pub fn section(title: &str, content: El, kit: &Kit) -> El {
    col()
        .gap(10.0)
        .w_full()
        .child(
            text(title)
                .font(kit.ty.ui_medium)
                .color(kit.c.text_muted)
                .pl(4.0),
        )
        .child(group(content, kit))
}

/// Creates a responsive preference row with a title, description, and control.
pub fn setting(title: &str, description: &str, control: El, kit: &Kit) -> El {
    row()
        .w_full()
        .flex_wrap()
        .min_h(60.0)
        .py(8.0)
        .center_y()
        .gap(12.0)
        .child(
            col()
                .grow(1.0)
                .min_w(200.0)
                .gap(5.0)
                .child(label(title, 14.0, kit.c.text_base).wrap().w_full())
                .child(label(description, 11.0, kit.c.text_muted).wrap().w_full()),
        )
        .child(control)
}

/// Creates an accessible normalized slider using caller-provided state accessors.
pub fn slider<S: 'static>(
    key: &str,
    title: &str,
    value: f32,
    access: fn(&S) -> f32,
    change: fn(&mut S, f32),
    kit: &Kit,
) -> El {
    const WIDTH: f32 = 180.0;
    const KNOB: f32 = 16.0;
    let travel = WIDTH - KNOB;
    el().key(key)
        .role("slider")
        .label(title)
        .focusable(true)
        .range_value(f64::from(value), 0.0, 1.0)
        .range_step(0.05)
        .a11y_value(format!("{:.0}%", value * 100.0))
        .size(WIDTH, 28.0)
        .on_step(move |state: &mut S, steps, _| {
            change(state, (access(state) + steps as f32 * 0.05).clamp(0.0, 1.0))
        })
        .on_drag(move |state: &mut S, drag: &mut Drag, tx| {
            let to_value = |x: f32| ((x - KNOB / 2.0) / travel).clamp(0.0, 1.0);
            match drag.phase {
                Phase::Down => {
                    drag.data = access(state);
                    change(state, to_value(drag.x));
                }
                Phase::Move => {
                    change(state, to_value(drag.x));
                    tx.instant();
                }
                Phase::Up if drag.cancelled() => change(state, drag.data),
                Phase::Up => {}
            }
        })
        .child(
            el().size(WIDTH, 4.0)
                .radius(2.0)
                .absolute(0.0, 12.0)
                .fill(kit.c.layer_01)
                .a11y_hidden(),
        )
        .child(
            el().size(KNOB / 2.0 + value * travel, 4.0)
                .radius(2.0)
                .absolute(0.0, 12.0)
                .fill(kit.c.bg_accent)
                .a11y_hidden(),
        )
        .child(
            el().key("knob")
                .size(KNOB, KNOB)
                .radius(KNOB / 2.0)
                .absolute(value * travel, 6.0)
                .fill(kit.c.text_base)
                .shadow(1.0, 3.0, Color::BLACK.alpha(0.25))
                .a11y_hidden(),
        )
}

/// Creates an accessible integer slider from a normalized state accessor.
pub fn integer_slider<S: 'static>(
    key: &str,
    title: &str,
    value: usize,
    range: (usize, usize, &str),
    access: fn(&S) -> f32,
    change: fn(&mut S, f32),
    kit: &Kit,
) -> El {
    let (min, max, unit) = range;
    let span = (max - min) as f32;
    let value = value.clamp(min, max);
    let normalized = (value - min) as f32 / span;
    slider::<S>(key, title, normalized, access, change, kit)
        .range_value(value as f64, min as f64, max as f64)
        .range_step(1.0)
        .a11y_value(format!("{value} {unit}"))
        .on_step(move |state: &mut S, steps, _| {
            change(state, (access(state) + steps as f32 / span).clamp(0.0, 1.0))
        })
        .on_step_limit(move |state: &mut S, max, _| change(state, if max { 1.0 } else { 0.0 }))
}

/// Creates a bounded, scrollable page body with caller-selected spacing.
pub fn page(content: El, padding: f32, max_width: f32) -> El {
    col()
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .scroll(ScrollSpec {
            stick_bottom: false,
        })
        .child(
            col()
                .key("page-content")
                .w_full()
                .padding(padding)
                .items_center()
                .child(content.w_full().max_w(max_width)),
        )
}

/// Creates monospaced text with a minimum readable font size.
pub fn mono(value: impl Into<String>, size: f32, color: Color) -> El {
    text(value)
        .font(TextStyle::new(size.max(12.0), 400.0).mono())
        .color(color)
}
