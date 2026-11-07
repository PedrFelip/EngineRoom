//! Mock candidate lines, preliminary triage and legal variation exploration.
use super::ui;
use crate::{
    review_data,
    state::{ReviewMode, ReviewTab, State},
};
use rune::{El, ScrollSpec, col};
use rune_kit::{Dialog, Kit, SegmentedControl, Switch};
pub fn view(s: &State, k: &Kit) -> El {
    if !s.is_mock() || !s.preferences.live_engine {
        return col().gap(14.0).child(ui::label("Linhas candidatas indisponíveis",16.0,k.c.text_base))
            .child(ui::label("Ative Demonstração e Análise para explorar alternativas legais com avaliações fictícias.",13.0,k.c.text_muted).wrap().w_full());
    }
    let lines = s.candidates();
    let mut content =
        col()
            .gap(12.0)
            .child(ui::label("Linhas candidatas · mock", 16.0, k.c.text_base));
    for (i, line) in lines.iter().take(s.preferences.lines.min(3)).enumerate() {
        let title = format!(
            "{}  {}   {}  d{}",
            i + 1,
            line.san.first().map_or("—", String::as_str),
            review_data::eval(line.cp),
            line.depth
        );
        content = content.child(
            ui::button(&format!("candidate-{i}"), &title, k)
                .w_full()
                .fill(if i == s.selected_candidate {
                    k.c.layer_01
                } else {
                    k.c.bg_base
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
                    format!(
                        "Chance das brancas: {:.1}% · dados fictícios",
                        50.0 + (line.cp as f32 / 5.0).clamp(-45.0, 45.0)
                    ),
                    12.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            )
            .child(
                ui::primary("explore-line", "▶ Explorar esta linha", k).on_press_tx(
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
    content = content.child(
        ui::button(
            "triage-toggle",
            if s.triage_open {
                "▾ Linhas da triagem"
            } else {
                "▸ Linhas da triagem"
            },
            k,
        )
        .on_press(|s: &mut State| s.triage_open = !s.triage_open),
    );
    if s.triage_open {
        content = content.child(ui::label(
            "Avaliação preliminar · mock",
            12.0,
            k.c.text_muted,
        ));
        for (i, line) in lines.iter().take(3).enumerate() {
            content = content.child(
                ui::button(
                    &format!("triage-{i}"),
                    &format!(
                        "{} · {} · d12",
                        line.san.first().map_or("—", String::as_str),
                        review_data::eval(line.cp - 12)
                    ),
                    k,
                )
                .on_press_tx(move |s: &mut State, tx| {
                    s.selected_candidate = i;
                    s.explore_candidate();
                    tx.wake_after(std::time::Duration::from_millis(50));
                }),
            );
        }
    }
    content
}
pub fn dialog(s: &State, k: &Kit, layers: &mut Vec<El>) {
    let switches = col()
        .gap(16.0)
        .child(ui::setting(
            "Análise durante a revisão",
            "Alternativas e setas no modo demonstração.",
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
                &["1", "2", "3"],
                Some(s.preferences.lines.saturating_sub(1).min(2)),
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
                "As opções alteram somente esta demonstração. Nenhuma análise real é executada.",
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
            &format!("{} s · preferência do mock", s.preferences.search_seconds),
            ui::slider(
                "dialog-time",
                "Tempo de busca",
                (s.preferences.search_seconds as f32 - 1.0) / 29.0,
                |s| (s.preferences.search_seconds as f32 - 1.0) / 29.0,
                |s, v| {
                    if s.preferences.live_engine {
                        s.preferences.search_seconds = (1.0 + v * 29.0).round() as usize;
                    }
                },
                k,
            ),
            k,
        ))
        .child(ui::setting(
            "Threads",
            &format!("{} threads · perfil fictício", s.preferences.threads),
            ui::slider(
                "dialog-threads",
                "Threads do mock",
                (s.preferences.threads as f32 - 1.0) / 7.0,
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
                .label("Adaptar ao computador · mock")
                .on_toggle(|s: &mut State| s.preferences.threads_auto = !s.preferences.threads_auto)
                .build(k)
                .key("dialog-threads-auto"),
        )
        .child(ui::setting(
            "Memória Hash",
            &format!("{} MB", s.preferences.hash_mb),
            ui::slider(
                "dialog-memory",
                "Memória do mock",
                (s.preferences.hash_mb as f32 - 16.0) / 1008.0,
                |s| (s.preferences.hash_mb as f32 - 16.0) / 1008.0,
                |s, v| {
                    if s.preferences.live_engine {
                        s.preferences.hash_mb = 16 + (v * 63.0).round() as usize * 16;
                    }
                },
                k,
            ),
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
pub fn toggle_mode(s: &mut State) {
    s.review_mode = if s.is_mock() {
        ReviewMode::Preview
    } else {
        ReviewMode::Mock
    };
    s.mock_status = crate::state::MockStatus::Ready;
    if !s.is_mock() && s.review_tab == ReviewTab::Summary {
        s.review_tab = ReviewTab::Moves;
    }
}
