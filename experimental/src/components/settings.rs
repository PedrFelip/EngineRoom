//! Settings sections mirroring Tauri: preferences, analysis and storage.
use super::ui;
use crate::{
    layout::Layout,
    state::{Appearance, SettingsSection, State},
};
use rune::{El, col, row};
use rune_kit::{Button, ControlSize, Kit, SegmentedControl, Select, Switch};

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
        s.preferences.appearance = if index == 0 {
            Appearance::Dark
        } else {
            Appearance::Light
        }
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
                "A paleta do OpenCode, em claro ou escuro.",
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
                    "Preferência visual; áudio ainda não conectado.",
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
    col().w_full().gap(26.0)
        .child(ui::section("Durante a revisão",col().gap(8.0)
            .child(ui::setting("Motor ao vivo","Analisar a posição selecionada durante a navegação.",toggle("live-engine","Motor ao vivo",s.preferences.live_engine,|s|s.preferences.live_engine= !s.preferences.live_engine,k),k))
            .child(ui::rule(k))
            .child(ui::setting("Feedback dos lances","Mostrar a classificação ao avançar pela partida.",toggle("feedback","Feedback dos lances",s.preferences.feedback,|s|s.preferences.feedback= !s.preferences.feedback,k),k))
            .child(ui::rule(k)).child(ui::setting("Linhas candidatas","Número de alternativas exibidas pelo motor.",lines,k)),k))
        .child(ui::section("Recursos do Stockfish",col().gap(8.0)
            .child(ui::setting("Threads",&format!("{} threads selecionadas",s.preferences.threads),
                ui::slider("threads","Threads do motor",(s.preferences.threads as f32-1.0)/7.0,|s|(s.preferences.threads as f32-1.0)/7.0,|s,v|s.preferences.threads=(1.0+v*7.0).round() as usize,k),k))
            .child(ui::rule(k)).child(ui::setting("Memória Hash",&format!("{} MB selecionados",s.preferences.hash_mb),
                ui::slider("hash","Memória Hash",(s.preferences.hash_mb as f32-64.0)/960.0,|s|(s.preferences.hash_mb as f32-64.0)/960.0,|s,v|s.preferences.hash_mb=64+(v*15.0).round() as usize*64,k),k)),k))
        .child(ui::label("Preferências mantidas nesta sessão. O backend ainda não recebe nem aplica esses valores.",11.0,k.c.text_muted).wrap().w_full())
}
fn storage(k: &Kit) -> El {
    ui::section("Dados locais",col().gap(18.0)
        .child(ui::label("Armazenamento ainda não conectado",16.0,k.c.text_base))
        .child(ui::label("O app Tauri guarda cache de posições e histórico em SQLite. Este frontend ainda não acessa esse banco.",13.0,k.c.text_muted).wrap().w_full())
        .child(row().gap(12.0).child(Button::new("Limpar cache").disabled(true).build(k).key("clear-cache"))
        .child(Button::new("Limpar histórico").disabled(true).build(k).key("clear-history"))),k)
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
    })
    .build(k)
    .key("settings-tabs")
    .max_w(500.0);
    let content = match s.section {
        SettingsSection::Preferences => preferences(s, k),
        SettingsSection::Analysis => analysis(s, k, layers),
        SettingsSection::Storage => storage(k),
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
        col().gap(24.0).child(header).child(tabs).child(content),
        layout,
    )
}
