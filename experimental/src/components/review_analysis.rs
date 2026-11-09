//! Candidate lines, preliminary triage and legal variation exploration.
use super::ui;
use crate::{review_data, state::State};
use rune::{Color, El, Icon, ScrollSpec, col};
use rune_kit::{Button, ButtonVariant, ControlSize, Dialog, Kit, SegmentedControl, Switch};

// Primer Octicons, MIT; see assets/OCTICONS-LICENSE.txt.
static PLAY: Icon = Icon {
    name: "play",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../assets/triangle-right-16.svg"),
        "</g>"
    ),
};
static CHECK: Icon = Icon {
    name: "check",
    view: 16.0,
    body: concat!(
        "<g fill=\"currentColor\" stroke=\"none\">",
        include_str!("../../assets/check-16.svg"),
        "</g>"
    ),
};
pub fn view(s: &State, k: &Kit) -> El {
    if s.real_result.is_none() || !s.preferences.live_engine {
        return col()
            .gap(14.0)
            .child(ui::label(
                "Linhas candidatas indisponíveis",
                16.0,
                k.c.text_base,
            ))
            .child(
                ui::label(
                    "As linhas aparecem após a análise. Ative Linhas para explorar alternativas.",
                    13.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            );
    }
    let lines = s.candidates();
    let mut content =
        col()
            .gap(12.0)
            .child(ui::label("Alternativas da posição", 16.0, k.c.text_base));
    for (i, line) in lines.iter().take(s.preferences.lines).enumerate() {
        let title = format!(
            "{}  {}   {}  d{}",
            i + 1,
            line.san.first().map_or("—", String::as_str),
            review_data::eval(line.cp),
            line.depth
        );
        let selected = i == s.selected_candidate;
        let button = Button::new(title)
            .variant(ButtonVariant::Ghost)
            .control_size(ControlSize::ExtraLarge);
        let button = if selected {
            button.icon(&CHECK)
        } else {
            button
        };
        content = content.child(
            button
                .build(k)
                .key(format!("candidate-{i}"))
                .checked(selected)
                .w_full()
                .border(
                    1.0,
                    if selected {
                        k.c.border_focus
                    } else {
                        Color::TRANSPARENT
                    },
                )
                .fill(if selected {
                    k.c.bg_accent.alpha(0.1)
                } else {
                    Color::TRANSPARENT
                })
                .on_press(move |s: &mut State| {
                    s.selected_candidate = i;
                    s.playing = false;
                }),
        );
    }
    if let Some(line) = lines.get(s.selected_candidate) {
        content = content
            .child(
                ui::mono(line.san.join("  "), 13.0, k.c.text_base)
                    .wrap()
                    .w_full(),
            )
            .child(
                ui::label(
                    format!("Chance das brancas: {:.1}%", {
                        s.live_result
                            .as_ref()
                            .or_else(|| {
                                if s.active_variation.is_none() {
                                    s.real_result.as_ref().and_then(|r| r.positions.get(s.ply))
                                } else {
                                    None
                                }
                            })
                            .and_then(|p| p.lines.get(s.selected_candidate))
                            .map_or(50.0, |l| l.win_pct as f32)
                    }),
                    12.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            )
            .child(
                ui::button_icon("explore-line", "Explorar esta linha", &PLAY, k).on_press_tx(
                    |s: &mut State, tx| {
                        s.explore_candidate();
                        tx.wake_after(std::time::Duration::from_millis(50));
                    },
                ),
            );
    } else {
        content = content.child(ui::label(
            "Posição final: sem lances legais.",
            13.0,
            k.c.text_muted,
        ));
    }
    content
}

pub fn dialog(s: &State, k: &Kit, layers: &mut Vec<El>) {
    let switches = col()
        .gap(16.0)
        .child(ui::setting(
            "Análise durante a revisão",
            "Alternativas e setas da posição analisada.",
            Switch::new(s.preferences.live_engine)
                .label("Ativar análise na revisão")
                .hide_label()
                .on_toggle(|s: &mut State| s.preferences.live_engine = !s.preferences.live_engine)
                .build(k)
                .key("dialog-live-engine"),
            k,
        ))
        .child(ui::setting(
            "Feedback dos lances",
            "Selos de classificação no tabuleiro e na lista.",
            Switch::new(s.preferences.feedback)
                .label("Feedback dos lances")
                .hide_label()
                .on_toggle(|s: &mut State| s.preferences.feedback = !s.preferences.feedback)
                .build(k)
                .key("dialog-feedback"),
            k,
        ))
        .child(ui::label("Linhas candidatas", 13.0, k.c.text_base))
        .child(
            SegmentedControl::new(
                &["1", "2", "3", "4", "5"],
                Some(s.preferences.lines.saturating_sub(1).min(4)),
            )
            .width(None)
            .label("Quantidade de linhas candidatas")
            .on_pick(|s: &mut State, i, _| {
                s.preferences.lines = i + 1;
                s.selected_candidate = 0;
            })
            .build(k)
            .key("dialog-lines"),
        )
        .child(
            ui::label(
                "As opções são aplicadas à próxima busca ao vivo.",
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        );
    let switches = switches
        .child(ui::rule(k))
        .child(ui::setting(
            "Tempo de busca",
            &format!("{} s", s.preferences.search_seconds),
            ui::integer_slider(
                "dialog-time",
                "Tempo de busca",
                s.preferences.search_seconds,
                (1, 30, "segundos"),
                |s| (s.preferences.search_seconds as f32 - 1.0) / 29.0,
                |s, v| {
                    if s.preferences.live_engine {
                        s.preferences.search_seconds = (1.0 + v * 29.0).round() as usize;
                    }
                },
                k,
            )
            .disabled(!s.preferences.live_engine),
            k,
        ))
        .child(ui::setting(
            "Threads",
            &format!("{} threads", s.preferences.threads),
            ui::integer_slider(
                "dialog-threads",
                "Threads",
                s.preferences.threads,
                (1, 8, "threads"),
                |s| (s.preferences.threads as f32 - 1.0) / 7.0,
                |s, v| {
                    if s.preferences.live_engine && !s.preferences.threads_auto {
                        s.preferences.threads = (1.0 + v * 7.0).round() as usize;
                    }
                },
                k,
            )
            .disabled(s.preferences.threads_auto || !s.preferences.live_engine)
            .opacity(if s.preferences.threads_auto { 0.4 } else { 1.0 }),
            k,
        ))
        .child(
            Switch::new(s.preferences.threads_auto)
                .label("Adaptar ao computador")
                .on_toggle(|s: &mut State| s.preferences.threads_auto = !s.preferences.threads_auto)
                .build(k)
                .key("dialog-threads-auto"),
        )
        .child(ui::setting(
            "Memória Hash",
            &format!("{} MB", s.preferences.hash_mb),
            ui::slider(
                "dialog-memory",
                "Memória",
                (s.preferences.hash_mb as f32 - 16.0) / 1008.0,
                |s| (s.preferences.hash_mb as f32 - 16.0) / 1008.0,
                |s, v| {
                    if s.preferences.live_engine {
                        s.preferences.hash_mb = 16 + (v * 63.0).round() as usize * 16;
                    }
                },
                k,
            )
            .range_value(s.preferences.hash_mb as f64, 16.0, 1024.0)
            .range_step(16.0)
            .a11y_value(format!("{} MB", s.preferences.hash_mb))
            .on_step(|s: &mut State, steps, _| {
                if s.preferences.live_engine {
                    s.preferences.hash_mb = (s.preferences.hash_mb as i64 + i64::from(steps) * 16)
                        .clamp(16, 1024) as usize;
                }
            })
            .on_step_limit(|s: &mut State, max, _| {
                if s.preferences.live_engine {
                    s.preferences.hash_mb = if max { 1024 } else { 16 };
                }
            })
            .disabled(!s.preferences.live_engine),
            k,
        ));
    Dialog::new("review-settings", "Análise na revisão")
        .open(s.analysis_settings_open)
        .width(560.0)
        .fit()
        .on_close(|s: &mut State| s.analysis_settings_open = false)
        .child(
            col()
                .key("review-settings-scroll")
                .h(360.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(switches.padding(16.0)),
        )
        .build(k, layers);
}
