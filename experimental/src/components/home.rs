//! The Tauri app's import → configure flow, composed from OpenCode controls.
use super::ui;
use crate::{
    game,
    layout::Layout,
    state::{ImportSource, Page, Profile, State},
};
use rune::{El, Icon, ScrollSpec, col, el, row};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, SegmentedControl, TextArea, TextInput};

static CHEVRON_LEFT: Icon = Icon {
    name: "chevron-left",
    view: 20.0,
    body: r#"<path d="M12 15L7 10L12 5" stroke="currentColor" stroke-linecap="square"/>"#,
};

fn steps(configure: bool, k: &Kit) -> El {
    row()
        .gap(14.0)
        .center_y()
        .child(ui::label(
            if configure {
                "✓  Partida importada"
            } else {
                "●  01  Importar partida"
            },
            12.0,
            if configure {
                k.c.text_muted
            } else {
                k.c.text_base
            },
        ))
        .child(ui::label("/", 12.0, k.c.text_faint))
        .child(ui::label(
            if configure {
                "●  02  Configurar análise"
            } else {
                "02  Configurar análise"
            },
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
    let source = match s.import_source {
        ImportSource::File => 0,
        ImportSource::Paste => 1,
        ImportSource::ChessCom => 2,
    };
    let sources = SegmentedControl::new(&["Arquivo PGN", "Colar PGN", "Chess.com"], Some(source))
        .label("Origem da partida")
        .width(None)
        .on_pick(|s: &mut State, index, _| {
            s.cancel_chesscom();
            s.import_source = match index {
                0 => ImportSource::File,
                1 => ImportSource::Paste,
                _ => ImportSource::ChessCom,
            };
            s.import_error = None;
            s.file_drag_over = false;
            if s.import_source == ImportSource::ChessCom
                && !s.preferences.chess_username.is_empty()
                && s.chess_games.is_empty()
                && !s.chess_searched
            {
                s.search_chesscom();
            }
        })
        .build(k)
        .key("import-source");
    let preview = if s.import_source != ImportSource::Paste || s.pgn.text.trim().is_empty() {
        el()
    } else {
        match game::parse(&s.pgn.text) {
            Ok(game) => ui::group(
                col()
                    .gap(6.0)
                    .child(
                        ui::label(
                            format!("{} × {}", game.white, game.black),
                            14.0,
                            k.c.text_base,
                        )
                        .wrap()
                        .w_full(),
                    )
                    .child(
                        ui::label(
                            format!("{} · {} meios-lances", game.event, game.moves.len()),
                            12.0,
                            k.c.text_muted,
                        )
                        .wrap()
                        .w_full(),
                    ),
                k,
            ),
            Err(error) => ui::label(
                s.import_error.as_deref().unwrap_or(&error),
                12.0,
                k.c.danger.fg,
            )
            .role("alert")
            .key("import-error")
            .wrap()
            .w_full(),
        }
    };
    let source_content = match s.import_source {
        ImportSource::Paste => col()
            .gap(14.0)
            .child(ui::label("Cole a notação da partida", 14.0, k.c.text_base))
            .child(
                ui::label(
                    "Inclua os lances e, se quiser, os dados dos jogadores.",
                    12.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            )
            .child(editor)
            .child(preview)
            .child(
                row()
                    .flex_wrap()
                    .gap(12.0)
                    .justify_between()
                    .center_y()
                    .child(
                        Button::new("Continuar")
                            .icon(&rune_theme_opencode::icons::CHEVRON_RIGHT)
                            .control_size(ControlSize::ExtraLarge)
                            .variant(ButtonVariant::Contrast)
                            .disabled(s.pgn.text.trim().is_empty())
                            .on_press(State::prepare)
                            .build(k)
                            .key("continue"),
                    ),
            ),
        ImportSource::File => col()
            .gap(12.0)
            .child(ui::label("Importar arquivo PGN", 16.0, k.c.text_base))
            .child(
                col()
                    .w_full()
                    .padding(28.0)
                    .gap(12.0)
                    .radius(12.0)
                    .border(
                        1.0,
                        if s.file_drag_over {
                            k.c.text_base
                        } else {
                            k.c.border_strong
                        },
                    )
                    .fill(k.c.bg_base)
                    .child(
                        ui::label(
                            "Solte seu arquivo .pgn ou .txt nesta área",
                            16.0,
                            k.c.text_base,
                        )
                        .wrap()
                        .w_full(),
                    )
                    .child(
                        ui::label(
                            "Arraste um arquivo para dentro desta caixa ou use o botão abaixo.",
                            12.0,
                            k.c.text_muted,
                        )
                        .wrap()
                        .w_full(),
                    )
                    .on_drop_hover(|s: &mut State, over, _| s.file_drag_over = over)
                    .on_drop_files(|s: &mut State, files, _| {
                        s.file_drag_over = false;
                        s.drop_pgn_files(files);
                    })
                    .child(
                        Button::new(if s.pending_picks > 0 {
                            "Escolhendo arquivo…"
                        } else if s.file_loading {
                            "Lendo arquivo…"
                        } else {
                            "Escolher arquivo…"
                        })
                        .control_size(ControlSize::ExtraLarge)
                        .variant(ButtonVariant::Contrast)
                        .disabled(s.file_loading || s.pending_picks > 0 || s.file_dialog.is_none())
                        .on_press(State::pick_pgn_file)
                        .build(k)
                        .key("choose-file"),
                    )
                    .key("pgn-drop-zone"),
            )
            .child(if s.file_dialog.is_none() {
                ui::label("Seletor de arquivos indisponível. Arraste um arquivo para a área acima ou cole o PGN.", 12.0, k.c.text_muted).wrap().w_full()
            } else { el() })
            .child(s.import_error.as_ref().map_or_else(el, |error| {
                ui::label(error, 12.0, k.c.danger.fg)
                    .role("alert")
                    .wrap()
                    .w_full()
            })),
        ImportSource::ChessCom => {
            let mut results = col().gap(8.0);
            let speeds = ["Todos", "bullet", "blitz", "rapid", "daily"];
            let mut visible = 0;
            let query = s.chess_filter.text.trim().to_lowercase();
            let mut selected_visible = None;
            for (index, game) in s.chess_games.iter().enumerate() {
                if s.chess_speed != 0 && game.time_class.as_deref() != Some(speeds[s.chess_speed]) {
                    continue;
                }
                if !query.is_empty()
                    && !game.white.username.to_lowercase().contains(&query)
                    && !game.black.username.to_lowercase().contains(&query)
                {
                    continue;
                }
                visible += 1;
                if s.chess_selected == Some(index) {
                    selected_visible = Some(index);
                }
                let white = game
                    .white
                    .username
                    .eq_ignore_ascii_case(s.chess_username.text.trim());
                let opponent = if white {
                    &game.black.username
                } else {
                    &game.white.username
                };
                let date = game
                    .pgn
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("[UTCDate \"")
                            .or_else(|| line.strip_prefix("[Date \""))
                    })
                    .and_then(|line| line.strip_suffix("\"]"))
                    .unwrap_or("Data desconhecida");
                let selected = s.chess_selected == Some(index);
                let title = format!("{} vs {}", if selected { "✓" } else { "○" }, opponent);
                let detail = format!(
                    "{} · {} · {} · {}",
                    if white { "Brancas" } else { "Pretas" },
                    game.time_class.as_deref().unwrap_or("Ritmo desconhecido"),
                    date,
                    game.result
                );
                let accessible = format!(
                    "{} · {}{}",
                    title,
                    detail,
                    if selected { " · Selecionada" } else { "" }
                );
                results = results.child(
                    Button::new(accessible)
                        .variant(if selected {
                            ButtonVariant::Contrast
                        } else {
                            ButtonVariant::Outline
                        })
                        .label_with(move |_font, ink| {
                            col()
                                .w_full()
                                .gap(4.0)
                                .child(ui::label(title, 14.0, ink).wrap().w_full())
                                .child(ui::label(detail, 12.0, ink).wrap().w_full())
                        })
                        .on_press(move |s: &mut State| s.chess_select(index))
                        .build(k)
                        .role("radio")
                        .checked(selected)
                        .h(64.0)
                        .w_full()
                        .key(format!("chess-game-{index}")),
                );
            }
            col()
                .gap(6.0)
                .child(ui::label("Suas partidas no Chess.com", 18.0, k.c.text_base))
                .child(
                    ui::label(
                        "Busque pelo seu usuário e escolha uma partida para analisar.",
                        12.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full(),
                )
                .child(
                    TextInput::new(&s.chess_username)
                        .label("Nome de usuário no Chess.com")
                        .placeholder("Seu nome de usuário")
                        .height(40.0)
                        .width(None)
                        .on_edit(|s: &mut State| &mut s.chess_username)
                        .on_change(|s: &mut State, _| s.chess_username_changed())
                        .build(k)
                        .key("chess-username"),
                )
                .child(if !s.preferences.chess_username.is_empty() {
                    ui::button("chess-forget", "Esquecer usuário salvo", k)
                        .on_press(State::forget_chesscom)
                } else {
                    el()
                })
                .child(
                    Button::new(if s.chess_loading {
                        "Buscando…"
                    } else {
                        "Buscar partidas"
                    })
                    .control_size(ControlSize::ExtraLarge)
                    .disabled(s.chess_username.text.trim().is_empty())
                    .on_press(State::search_chesscom)
                    .build(k)
                    .key("chess-search"),
                )
                .child(if s.chess_loading {
                    ui::button("chess-cancel", "Cancelar", k).on_press(State::cancel_chesscom)
                } else {
                    el()
                })
                .child(s.import_error.as_ref().map_or_else(el, |error| {
                    ui::label(error, 12.0, k.c.danger.fg)
                        .role("alert")
                        .wrap()
                        .w_full()
                }))
                .child(s.chess_warning.as_ref().map_or_else(el, |warning| {
                    ui::label(warning, 12.0, k.c.text_muted).wrap().w_full()
                }))
                .child(if s.chess_loading {
                    ui::label("Buscando partidas…", 12.0, k.c.text_muted)
                        .wrap()
                        .w_full()
                } else if s.chess_searched && s.chess_games.is_empty() && s.import_error.is_none() {
                    ui::label(
                        "Nenhuma partida encontrada para este usuário.",
                        12.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full()
                } else if !s.chess_games.is_empty() && visible == 0 {
                    ui::label(
                        "Nenhuma partida corresponde aos filtros nos resultados carregados.",
                        12.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full()
                } else {
                    el()
                })
                .child(if !s.chess_games.is_empty() {
                    SegmentedControl::new(
                        &["Todos", "Bullet", "Blitz", "Rapid", "Diário"],
                        Some(s.chess_speed),
                    )
                    .label("Filtrar por ritmo")
                    .width(None)
                    .on_pick(|s: &mut State, i, _| {
                        s.chess_speed = i;
                        let query = s.chess_filter.text.clone();
                        s.chess_sync_selection(i, &query);
                    })
                    .build(k)
                    .key("chess-speed")
                } else {
                    el()
                })
                .child(if !s.chess_games.is_empty() {
                    TextInput::new(&s.chess_filter)
                        .label("Filtrar partidas por jogador")
                        .placeholder("Filtrar por jogador")
                        .height(36.0)
                        .width(None)
                        .on_edit(|s: &mut State| &mut s.chess_filter)
                        .on_change(|s: &mut State, _| {
                            s.chess_sync_selection(s.chess_speed, &s.chess_filter.text.clone())
                        })
                        .build(k)
                        .key("chess-filter")
                } else {
                    el()
                })
                .child(if !s.chess_games.is_empty() {
                    ui::label(
                        format!(
                            "{} de {} partidas · {}{}",
                            visible,
                            s.chess_games.len(),
                            if selected_visible.is_some() {
                                "partida selecionada"
                            } else {
                                "escolha uma partida"
                            },
                            if visible > 2 {
                                " · role a lista para ver mais"
                            } else {
                                ""
                            }
                        ),
                        12.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full()
                } else {
                    el()
                })
                .child(if let Some(index) = selected_visible {
                    Button::new(format!(
                        "Continuar: {} × {}",
                        s.chess_games[index].white.username, s.chess_games[index].black.username
                    ))
                    .control_size(ControlSize::ExtraLarge)
                    .on_press(move |s: &mut State| s.select_chesscom(index))
                    .build(k)
                    .key("chess-continue")
                } else {
                    el()
                })
                .child(if !s.chess_games.is_empty() {
                    col()
                        .key("chess-results-scroll")
                        .h(if layout.height < 650.0 { 156.0 } else { 260.0 })
                        .w_full()
                        .scroll(ScrollSpec {
                            stick_bottom: false,
                        })
                        .child(results)
                } else {
                    el()
                })
                .child(if s.chess_cursor.is_some() {
                    Button::new("Mais partidas")
                        .disabled(s.chess_loading)
                        .on_press(State::next_chesscom)
                        .build(k)
                        .key("chess-next")
                } else {
                    el()
                })
        }
    };
    let content = col()
        .w_full()
        .gap(if s.import_source == ImportSource::ChessCom {
            12.0
        } else {
            24.0
        })
        .child(steps(false, k))
        .child(if s.import_source == ImportSource::ChessCom {
            el()
        } else {
            col()
                .gap(10.0)
                .child(ui::heading("Sua próxima descoberta começa aqui.", k))
                .child(
                    ui::label(
                        "Escolha uma partida para explorar suas decisões, lance a lance.",
                        13.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .w_full(),
                )
        })
        .child(ui::section(
            "Importar partida",
            col()
                .gap(if s.import_source == ImportSource::ChessCom {
                    10.0
                } else {
                    20.0
                })
                .child(sources)
                .child(ui::rule(k))
                .child(source_content),
            k,
        ));
    let content = content.child(
        ui::label("Análise local com Stockfish.", 12.0, k.c.text_muted)
            .wrap()
            .w_full(),
    );
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
        Profile::Manual => "Escolha o orçamento e as linhas candidatas para esta prévia.",
    };
    let mut settings = col()
        .gap(20.0)
        .child(profiles)
        .child(ui::label(
            match s.profile {
                Profile::Fast => "Uma visão geral, mais rápida",
                Profile::Deep => "Mais atenção aos momentos críticos",
                Profile::Manual => "Ajustes para sua análise",
            },
            16.0,
            k.c.text_base,
        ))
        .child(ui::label(description, 13.0, k.c.text_muted).wrap().w_full());
    if s.profile == Profile::Manual {
        settings = settings.child(ui::rule(k)).child(
            SegmentedControl::new(&["Profundidade", "Tempo"], Some(usize::from(s.manual_time)))
                .label("Limite por posição")
                .on_pick(|s: &mut State, index, _| s.manual_time = index == 1)
                .build(k)
                .key("manual-limit"),
        );
        if s.manual_time {
            settings = settings.child(ui::setting(
                "Tempo por posição",
                &format!("{} segundos", s.preferences.search_seconds),
                ui::integer_slider(
                    "manual-time",
                    "Tempo por posição",
                    s.preferences.search_seconds,
                    (1, 30, "segundos"),
                    |s| (s.preferences.search_seconds as f32 - 1.0) / 29.0,
                    |s, v| s.preferences.search_seconds = (1.0 + v * 29.0).round() as usize,
                    k,
                ),
                k,
            ));
        } else {
            settings = settings.child(ui::setting(
                "Profundidade",
                &format!("{} níveis", s.depth),
                ui::integer_slider(
                    "depth",
                    "Profundidade",
                    s.depth,
                    (12, 30, "níveis"),
                    |s| (s.depth as f32 - 12.0) / 18.0,
                    |s, v| s.depth = (12.0 + v * 18.0).round() as usize,
                    k,
                ),
                k,
            ));
        }
        settings = settings.child(ui::setting(
            "Linhas candidatas",
            &s.preferences.lines.to_string(),
            ui::integer_slider(
                "manual-lines",
                "Linhas candidatas",
                s.preferences.lines,
                (1, 5, "linhas"),
                |s| (s.preferences.lines as f32 - 1.0) / 4.0,
                |s, v| s.preferences.lines = (1.0 + v * 4.0).round() as usize,
                k,
            ),
            k,
        ));
    }
    ui::page(
        col()
            .gap(24.0)
            .child(steps(true, k))
            .child(
                col()
                    .gap(10.0)
                    .child(ui::heading("Prepare sua revisão", k))
                    .child(
                        ui::label(
                            "Escolha como quer explorar esta partida.",
                            13.0,
                            k.c.text_muted,
                        )
                        .wrap()
                        .w_full(),
                    ),
            )
            .child(ui::group(
                col()
                    .gap(10.0)
                    .child(
                        ui::label(
                            format!("{} × {}", s.game.white, s.game.black),
                            18.0,
                            k.c.text_base,
                        )
                        .wrap()
                        .w_full(),
                    )
                    .child(
                        ui::label(
                            format!("{}  ·  {} meios-lances", s.game.event, s.game.moves.len()),
                            12.0,
                            k.c.text_muted,
                        )
                        .wrap()
                        .w_full(),
                    ),
                k,
            ))
            .child(ui::section("Perfil de análise", settings, k))
            .child(
                ui::label(
                    "Escolha o perfil para analisar a partida localmente com Stockfish.",
                    12.0,
                    k.c.text_muted,
                )
                .wrap()
                .w_full(),
            )
            .child(
                row()
                    .justify_between()
                    .child(
                        ui::button_icon("import-back", "Voltar", &CHEVRON_LEFT, k)
                            .on_press(|s: &mut State| s.navigate(Page::Home)),
                    )
                    .child(
                        ui::button_trailing(
                            "start-review",
                            "Abrir revisão",
                            ButtonVariant::Contrast,
                            k,
                        )
                        .on_press(|s: &mut State| s.start_review()),
                    ),
            )
            .child(el().h(8.0)),
        layout,
    )
}
