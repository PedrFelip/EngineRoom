//! The Tauri app's import → configure flow, composed from OpenCode controls.
use super::ui;
use crate::{
    layout::Layout,
    state::{Page, Profile, State},
};
use rune::{El, col, el, row};
use rune_kit::{Button, ButtonVariant, Card, ControlSize, Kit, SegmentedControl, TextArea};

fn steps(configure: bool, k: &Kit) -> El {
    row()
        .gap(14.0)
        .center_y()
        .child(ui::label(
            "01  Importar partida",
            12.0,
            if configure {
                k.c.text_muted
            } else {
                k.c.text_base
            },
        ))
        .child(ui::label("/", 12.0, k.c.text_faint))
        .child(ui::label(
            "02  Configurar análise",
            12.0,
            if configure {
                k.c.text_base
            } else {
                k.c.text_muted
            },
        ))
}
pub fn view(s: &State, k: &Kit, layout: Layout) -> El {
    let mut editor_kit = k.clone();
    editor_kit.ty.ui = k.ty.mono;
    let editor = TextArea::new(&s.pgn)
        .label("PGN da partida")
        .placeholder("Cole o PGN da sua partida…\n\n1. e4 e5 2. Nf3 Nc6 …")
        .rows(if layout.height < 650.0 { 6 } else { 9 })
        .max_height(260.0)
        .width(None)
        .on_edit(|s: &mut State| &mut s.pgn)
        .on_change(|s: &mut State, _| s.import_error = None)
        .build(&editor_kit)
        .key("pgn-editor");
    let actions = row()
        .flex_wrap()
        .gap(12.0)
        .justify_between()
        .center_y()
        .child(ui::button("load-demo", "Carregar exemplo", k).on_press(State::load_demo))
        .child(
            Button::new("Continuar  →")
                .control_size(ControlSize::ExtraLarge)
                .variant(ButtonVariant::Contrast)
                .disabled(s.pgn.text.trim().is_empty())
                .on_press(State::prepare)
                .build(k)
                .key("continue"),
        );
    let mut content = col()
        .w_full()
        .gap(24.0)
        .child(steps(false, k))
        .child(
            col()
                .gap(10.0)
                .child(ui::heading("Uma nova perspectiva sobre seu jogo.", k))
                .child(
                    ui::label(
                        "Importe uma partida e explore cada decisão, lance a lance.",
                        13.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full(),
                ),
        )
        .child(ui::section(
            "Partida em PGN",
            col().gap(16.0).child(editor).child(actions),
            k,
        ));
    if let Some(error) = &s.import_error {
        content = content.child(
            ui::label(error, 12.0, k.c.danger.fg)
                .role("alert")
                .key("import-error")
                .wrap()
                .w_full(),
        );
    }
    content=content.child(Card::new().title("Seu jogo fica no seu computador",None)
        .description("A prévia do PGN funciona localmente. A análise com Stockfish, a importação de arquivos e a busca no Chess.com ainda precisam da conexão com o backend.")
        .build(k).key("local-preview-notice"));
    ui::page(content, layout)
}

pub fn configure(s: &State, k: &Kit, layout: Layout) -> El {
    let selection = match s.profile {
        Profile::Fast => 0,
        Profile::Deep => 1,
        Profile::Manual => 2,
    };
    let profiles = SegmentedControl::new(&["Rápido", "Profundo", "Manual"], Some(selection))
        .width(None)
        .label("Perfil de análise")
        .on_pick(|s: &mut State, index, _| {
            s.profile = match index {
                0 => Profile::Fast,
                1 => Profile::Deep,
                _ => Profile::Manual,
            }
        })
        .build(k)
        .key("profile");
    let description = match s.profile {
        Profile::Fast => "Revisão automática para encontrar os momentos importantes da partida.",
        Profile::Deep => "Revisão automática com mais atenção às decisões críticas.",
        Profile::Manual => "Escolha a profundidade para a futura integração com o motor.",
    };
    let mut settings = col()
        .gap(20.0)
        .child(profiles)
        .child(ui::label(description, 13.0, k.c.text_muted).wrap().w_full());
    if s.profile == Profile::Manual {
        settings = settings.child(ui::rule(k)).child(ui::setting(
            "Profundidade",
            &format!("{} níveis", s.depth),
            ui::slider(
                "depth",
                "Profundidade",
                (s.depth as f32 - 12.0) / 18.0,
                |s| (s.depth as f32 - 12.0) / 18.0,
                |s, v| s.depth = (12.0 + v * 18.0).round() as usize,
                k,
            ),
            k,
        ));
    }
    ui::page(col().gap(24.0)
        .child(steps(true,k)).child(ui::heading("Prepare sua revisão",k))
        .child(ui::group(col().gap(10.0).child(ui::label(format!("{} × {}",s.game.white,s.game.black),18.0,k.c.text_base).wrap().w_full())
            .child(ui::label(format!("{}  ·  {} meios-lances",s.game.event,s.game.moves.len()),12.0,k.c.text_muted).wrap().w_full()),k))
        .child(ui::section("Perfil de análise",settings,k))
        .child(ui::label("Nesta etapa do experimento, o perfil é uma preferência da interface. A revisão abre uma prévia navegável, sem executar Stockfish.",12.0,k.c.text_muted).wrap().w_full())
        .child(row().justify_between().child(ui::button("import-back","←  Voltar",k).on_press(|s:&mut State|s.navigate(Page::Home)))
            .child(ui::primary("start-review","Abrir revisão  →",k).on_press(|s:&mut State|s.navigate(Page::Review))))
        .child(el().h(8.0)),layout)
}
