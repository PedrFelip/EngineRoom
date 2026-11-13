//! EngineRoom composition helpers over rune-kit, shared by every screen.
use crate::{layout::Layout, state::State};
use rune::{Color, Drag, El, Icon, Phase, ScrollSpec, TextStyle, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit};

pub fn label(value: impl Into<String>, size: f32, color: Color) -> El {
    text(value)
        .font(TextStyle::new(size.max(12.0), 400.0))
        .color(color)
}
pub fn heading(value: &str, k: &Kit) -> El {
    text(value)
        .font(TextStyle::new(24.0, 600.0))
        .color(k.c.text_base)
        .wrap()
        .min_w(0.0)
}
pub fn rule(k: &Kit) -> El {
    el().h(1.0).w_full().fill(k.c.border_base).a11y_hidden()
}
pub fn button(key: &str, title: &str, k: &Kit) -> El {
    Button::new(title)
        .control_size(ControlSize::ExtraLarge)
        .build(k)
        .key(key)
}
pub fn button_icon(key: &str, title: &str, icon: &'static Icon, k: &Kit) -> El {
    Button::new(title)
        .icon(icon)
        .control_size(ControlSize::ExtraLarge)
        .build(k)
        .key(key)
}
pub fn button_trailing(key: &str, title: &str, variant: ButtonVariant, k: &Kit) -> El {
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
        .build(k)
        .key(key)
}
pub fn group(content: El, k: &Kit) -> El {
    col()
        .w_full()
        .padding(16.0)
        .radius(12.0)
        .border(1.0, k.c.border_base)
        .fill(k.c.bg_base)
        .child(content)
}
pub fn section(title: &str, content: El, k: &Kit) -> El {
    col()
        .gap(10.0)
        .w_full()
        .child(
            text(title)
                .font(k.ty.ui_medium)
                .color(k.c.text_muted)
                .pl(4.0),
        )
        .child(group(content, k))
}
pub fn setting(title: &str, description: &str, control: El, k: &Kit) -> El {
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
                .child(label(title, 14.0, k.c.text_base).wrap().w_full())
                .child(label(description, 11.0, k.c.text_muted).wrap().w_full()),
        )
        .child(control)
}
/// A native Rune slider with pointer, keyboard, accessibility and drag-cancellation support.
pub fn slider(
    key: &str,
    title: &str,
    value: f32,
    access: fn(&State) -> f32,
    change: fn(&mut State, f32),
    k: &Kit,
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
        .on_step(move |s: &mut State, steps, _| {
            change(s, (access(s) + steps as f32 * 0.05).clamp(0.0, 1.0))
        })
        .on_drag(move |s: &mut State, d: &mut Drag, tx| {
            let to_value = |x: f32| ((x - KNOB / 2.0) / travel).clamp(0.0, 1.0);
            match d.phase {
                Phase::Down => {
                    d.data = access(s);
                    change(s, to_value(d.x));
                }
                Phase::Move => {
                    change(s, to_value(d.x));
                    tx.instant();
                }
                Phase::Up => {
                    if d.cancelled() {
                        change(s, d.data);
                    }
                }
            }
        })
        .child(
            el().size(WIDTH, 4.0)
                .radius(2.0)
                .absolute(0.0, 12.0)
                .fill(k.c.layer_01)
                .a11y_hidden(),
        )
        .child(
            el().size(KNOB / 2.0 + value * travel, 4.0)
                .radius(2.0)
                .absolute(0.0, 12.0)
                .fill(k.c.bg_accent)
                .a11y_hidden(),
        )
        .child(
            el().key("knob")
                .size(KNOB, KNOB)
                .radius(KNOB / 2.0)
                .absolute(value * travel, 6.0)
                .fill(k.c.text_base)
                .shadow(1.0, 3.0, Color::BLACK.alpha(0.25))
                .a11y_hidden(),
        )
}

pub fn integer_slider(
    key: &str,
    title: &str,
    value: usize,
    range: (usize, usize, &str),
    access: fn(&State) -> f32,
    change: fn(&mut State, f32),
    k: &Kit,
) -> El {
    let (min, max, unit) = range;
    let span = (max - min) as f32;
    let value = value.clamp(min, max);
    let normalized = (value - min) as f32 / span;
    slider(key, title, normalized, access, change, k)
        .range_value(value as f64, min as f64, max as f64)
        .range_step(1.0)
        .a11y_value(format!("{value} {unit}"))
        .on_step(move |s: &mut State, steps, _| {
            change(s, (access(s) + steps as f32 / span).clamp(0.0, 1.0))
        })
        .on_step_limit(move |s: &mut State, max, _| change(s, if max { 1.0 } else { 0.0 }))
}

/// A bounded form viewport. Only its content grows, so short windows scroll naturally.
pub fn page(content: El, layout: Layout) -> El {
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
                .padding(layout.padding())
                .items_center()
                .child(content.w_full().max_w(800.0)),
        )
}
pub fn mono(value: impl Into<String>, size: f32, color: Color) -> El {
    text(value)
        .font(TextStyle::new(size.max(12.0), 400.0).mono())
        .color(color)
}
