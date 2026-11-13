//! Settings sections mirroring Tauri: preferences, analysis and storage.
use super::ui;
use crate::{
    layout::Layout,
    state::{Appearance, SettingsSection, State},
};
use rune::{El, col, row};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, SegmentedControl, Select, Switch};

fn toggle(key: &str, title: &str, value: bool, change: fn(&mut State), k: &Kit) -> El {
    Switch::new(value)
        .control_size(ControlSize::Small)
        .label(title)
        .hide_label()
        .on_toggle(change)
        .build(k)
        .key(key)
}
fn preferences(s: &State, k: &Kit) -> El {
    let theme = SegmentedControl::new(
        &["Escuro", "Claro"],
        Some(usize::from(s.preferences.appearance == Appearance::Light)),
    )
    .label("Tema da interface")
    .width(Some(220.0))
    .on_pick(|s: &mut State, index, _| {
        s.set_appearance(if index == 0 {
            Appearance::Dark
        } else {
            Appearance::Light
        });
    })
    .build(k)
    .key("appearance");
    let sound = toggle(
        "sound",
        "Som ao avançar lance",
        s.preferences.sound,
        |s| s.preferences.sound = !s.preferences.sound,
        k,
    );
    let volume = ui::slider(
        "volume",
        "Volume do som",
        s.preferences.volume,
        |s| s.preferences.volume,
        |s, v| {
            if s.preferences.sound {
                s.preferences.volume = v;
            }
        },
        k,
    );
    let volume = if s.preferences.sound {
        volume
    } else {
        volume.disabled(true).opacity(0.4)
    };
    let volume = row().gap(14.0).center_y().child(volume).child(
        ui::label(
            format!("{:.0}%", s.preferences.volume * 100.0),
            12.0,
            k.c.text_muted,
        )
        .w(34.0),
    );
    col()
        .w_full()
        .gap(26.0)
        .child(ui::section(
            "Aparência",
            col().child(ui::setting(
                "Tema",
                "Escolha a aparência clara ou escura.",
                theme,
                k,
            )),
            k,
        ))
        .child(ui::section(
            "Som",
            col()
                .gap(8.0)
                .child(ui::setting(
                    "Som ao avançar lance",
                    "Toca um som ao avançar pela partida.",
                    sound,
                    k,
                ))
                .child(ui::rule(k))
                .child(ui::setting(
                    "Volume",
                    "Intensidade dos sons de movimentação.",
                    volume,
                    k,
                )),
            k,
        ))
        .child(ui::section(
            "Movimento",
            col().child(ui::setting(
                "Reduzir animações",
                "Mantém as transições discretas ao navegar.",
                toggle(
                    "reduce-motion",
                    "Reduzir animações",
                    s.preferences.reduced_motion,
                    |s| s.preferences.reduced_motion = !s.preferences.reduced_motion,
                    k,
                ),
                k,
            )),
            k,
        ))
}
fn analysis(s: &State, k: &Kit, layers: &mut Vec<El>) -> El {
    let line = s.preferences.lines.to_string();
    let lines = Select::new("lines", &["1", "2", "3", "4", "5"])
        .label("Linhas candidatas")
        .width(140.0)
        .current(Some(&line))
        .open(s.select_open)
        .on_toggle(|s: &mut State| s.select_open = !s.select_open)
        .on_close(|s: &mut State| s.select_open = false)
        .on_choose(|s: &mut State, v, _| {
            s.preferences.lines = v.parse().unwrap_or(1);
            s.select_open = false;
        })
        .build(k, layers);
    col()
        .w_full()
        .gap(26.0)
        .child(ui::section(
            "Durante a revisão",
            col()
                .gap(8.0)
                .child(ui::setting(
                    "Motor ao vivo",
                    "Analisar a posição selecionada durante a navegação.",
                    toggle(
                        "live-engine",
                        "Motor ao vivo",
                        s.preferences.live_engine,
                        State::toggle_live_engine,
                        k,
                    ),
                    k,
                ))
                .child(ui::rule(k))
                .child(ui::setting(
                    "Feedback dos lances",
                    "Mostrar a classificação ao avançar pela partida.",
                    toggle(
                        "feedback",
                        "Feedback dos lances",
                        s.preferences.feedback,
                        |s| s.preferences.feedback = !s.preferences.feedback,
                        k,
                    ),
                    k,
                ))
                .child(ui::rule(k))
                .child(ui::setting(
                    "Linhas candidatas",
                    "Número de alternativas exibidas pelo motor.",
                    lines,
                    k,
                )),
            k,
        ))
        .child(ui::section(
            "Recursos do Stockfish",
            col()
                .gap(8.0)
                .child(ui::setting(
                    "Threads",
                    &format!("{} threads selecionadas", s.preferences.threads),
                    ui::slider(
                        "threads",
                        "Threads do motor",
                        (s.preferences.threads as f32 - 1.0) / 7.0,
                        |s| (s.preferences.threads as f32 - 1.0) / 7.0,
                        |s, v| s.preferences.threads = (1.0 + v * 7.0).round() as usize,
                        k,
                    ),
                    k,
                ))
                .child(ui::rule(k))
                .child(ui::setting(
                    "Memória Hash",
                    &format!("{} MB selecionados", s.preferences.hash_mb),
                    ui::slider(
                        "hash",
                        "Memória Hash",
                        (s.preferences.hash_mb as f32 - 64.0) / 960.0,
                        |s| (s.preferences.hash_mb as f32 - 64.0) / 960.0,
                        |s, v| s.preferences.hash_mb = 64 + (v * 15.0).round() as usize * 64,
                        k,
                    ),
                    k,
                )),
            k,
        ))
        .child(ui::section("Verificar Stockfish", probe(s, k), k))
        .child(
            ui::label(
                "As preferências são salvas neste computador.",
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
}
fn probe(s: &State, k: &Kit) -> El {
    let running = matches!(s.engine_probe, crate::engine_probe::Status::Running);
    let mut content = col()
        .gap(10.0)
        .child(
            ui::label(
                "Verifique se o Stockfish está disponível para analisar partidas.",
                12.0,
                k.c.text_muted,
            )
            .wrap()
            .w_full(),
        )
        .child(
            Button::new(if running {
                "Testando…"
            } else {
                "Testar Stockfish"
            })
            .control_size(ControlSize::ExtraLarge)
            .disabled(running || s.bridge.is_none())
            .build(k)
            .key("test-engine")
            .on_press(|s: &mut State| s.start_engine_probe()),
        );
    match &s.engine_probe {
        crate::engine_probe::Status::Success(name) => {
            content = content.child(
                ui::label(
                    name.as_deref().unwrap_or("Stockfish respondeu"),
                    12.0,
                    k.c.success.fg,
                )
                .wrap()
                .w_full(),
            );
        }
        crate::engine_probe::Status::Error(error) => {
            content = content.child(
                ui::label(error, 12.0, k.c.danger.fg)
                    .role("alert")
                    .wrap()
                    .w_full(),
            );
        }
        crate::engine_probe::Status::Idle | crate::engine_probe::Status::Running => {}
    }
    content
}
fn readable_bytes(value: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let value = value as f64;
    if value >= MB {
        format!("{:.1} MB", value / MB)
    } else if value >= KB {
        format!("{:.1} KB", value / KB)
    } else {
        format!("{value:.1} B")
    }
}
fn storage_action(key: &str, title: &str, disabled: bool, k: &Kit) -> El {
    Button::new(title)
        .control_size(ControlSize::ExtraLarge)
        .disabled(disabled)
        .build(k)
        .key(key)
}
fn storage_destructive(key: &str, title: &str, disabled: bool, k: &Kit) -> El {
    Button::new(title)
        .icon(&super::icons::TRASH)
        .control_size(ControlSize::ExtraLarge)
        .disabled(disabled)
        .build(k)
        .key(key)
}
fn storage(s: &State, k: &Kit) -> El {
    let disabled = s.storage_loading || s.bridge.is_none();
    let mut details = col().w_full().gap(8.0);
    if s.storage_loading {
        details = details.child(ui::label("Carregando dados locais…", 12.0, k.c.text_muted));
    }
    if let Some(error) = &s.storage_error {
        details = details.child(
            ui::label(error, 12.0, k.c.danger.fg)
                .role("alert")
                .wrap()
                .w_full(),
        );
    }
    if let Some(message) = &s.storage_message {
        details = details.child(ui::label(message, 12.0, k.c.success.fg).wrap().w_full());
    }
    if let Some(stats) = &s.storage_stats {
        details = details
            .child(
                ui::setting(
                    "Cache de posições",
                    "Avaliações do Stockfish reutilizadas entre revisões.",
                    ui::label(readable_bytes(stats.cache_bytes), 13.0, k.c.text_base),
                    k,
                )
                .key("storage-cache"),
            )
            .child(ui::rule(k))
            .child(
                ui::setting(
                    "Histórico de revisões",
                    "Partidas revisadas guardadas no banco local.",
                    ui::label(readable_bytes(stats.games_bytes), 13.0, k.c.text_base),
                    k,
                )
                .key("storage-history"),
            )
            .child(ui::rule(k))
            .child(
                ui::setting(
                    "Arquivo de dados",
                    "Tamanho total dos dados locais no disco.",
                    ui::label(readable_bytes(stats.db_bytes), 13.0, k.c.text_base),
                    k,
                )
                .key("storage-database"),
            );
    } else if !s.storage_loading && s.storage_error.is_none() {
        details = details.child(ui::label(
            "Nenhum dado local encontrado.",
            12.0,
            k.c.text_muted,
        ));
    }
    let actions = row()
        .flex_wrap()
        .gap(8.0)
        .child(
            storage_action("refresh-storage", "Atualizar", disabled, k)
                .on_press(|s: &mut State| s.refresh_storage()),
        )
        .child(
            storage_destructive("clear-cache", "Limpar cache", disabled, k)
                .on_press(|s: &mut State| s.clear_storage(false)),
        )
        .child(
            storage_destructive("clear-history", "Limpar histórico", disabled, k)
                .on_press(|s: &mut State| s.request_clear_history()),
        );
    let mut maintenance = col().w_full().gap(8.0)
        .child(ui::label("Limpar o cache remove avaliações reutilizáveis. Limpar o histórico remove as revisões salvas.", 12.0, k.c.text_muted).wrap().w_full())
        .child(actions);
    if s.confirm_clear_history {
        maintenance = maintenance.child(ui::rule(k)).child(ui::group(
            col()
                .gap(10.0)
                .child(ui::label(
                    "Apagar todo o histórico de revisões?",
                    14.0,
                    k.c.text_base,
                ))
                .child(
                    ui::label(
                        "As partidas salvas e suas análises serão removidas deste computador. O cache de posições é mantido. Esta ação não pode ser desfeita.",
                        12.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full(),
                )
                .child(
                    row()
                        .flex_wrap()
                        .gap(8.0)
                        .child(
                            storage_action("cancel-clear-history", "Cancelar", disabled, k)
                                .on_press(|s: &mut State| s.cancel_clear_history()),
                        )
                        .child(
                            Button::new("Apagar histórico")
                                .icon(&super::icons::TRASH)
                                .variant(ButtonVariant::Danger)
                                .control_size(ControlSize::ExtraLarge)
                                .disabled(disabled)
                                .build(k)
                                .key("confirm-clear-history")
                                .on_press(|s: &mut State| s.clear_storage(true)),
                        ),
                ),
            k,
        ));
    }
    col()
        .w_full()
        .gap(26.0)
        .child(ui::section("Dados locais", details, k))
        .child(ui::section("Manutenção", maintenance, k))
}
pub fn view(s: &State, k: &Kit, layout: Layout, layers: &mut Vec<El>) -> El {
    let selected = match s.section {
        SettingsSection::Preferences => 0,
        SettingsSection::Analysis => 1,
        SettingsSection::Storage => 2,
    };
    let tabs = SegmentedControl::new(
        &["Preferências", "Análise", "Armazenamento"],
        Some(selected),
    )
    .label("Seção de configurações")
    .width(None)
    .on_pick(|s: &mut State, index, _| {
        s.section = match index {
            0 => SettingsSection::Preferences,
            1 => SettingsSection::Analysis,
            _ => SettingsSection::Storage,
        };
        s.select_open = false;
        if index == 2 {
            s.refresh_storage();
        }
    })
    .build(k)
    .key("settings-tabs")
    .max_w(500.0);
    let content = match s.section {
        SettingsSection::Preferences => preferences(s, k),
        SettingsSection::Analysis => analysis(s, k, layers),
        SettingsSection::Storage => storage(s, k),
    };
    let header = row()
        .gap(16.0)
        .center_y()
        .child(ui::heading("Configurações", k).grow(1.0))
        .child(
            ui::button("settings-close", "Concluir", k)
                .on_press(|s: &mut State| s.navigate(s.previous_page)),
        );
    ui::page(
        col()
            .gap(24.0)
            .child(header)
            .child(tabs)
            .child(s.preference_error.as_ref().map_or_else(rune::el, |error| {
                ui::label(error, 12.0, k.c.danger.fg)
                    .role("alert")
                    .wrap()
                    .w_full()
            }))
            .child(content),
        layout,
    )
}
