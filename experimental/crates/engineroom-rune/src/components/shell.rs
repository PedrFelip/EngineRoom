//! Native Rune desktop shell with a resizable sidebar.
use super::ui;
use crate::state::{Page, State};
use rune::{Color, Cursor, Cx, Drag, El, Phase, col, el, row};
use rune_kit::{
    ControlSize, IconButton, IconButtonVariant, Kit,
    shell::{SIDEBAR, Sidebar, SidebarRow, SidebarSection},
};
use rune_theme_opencode::icons;
fn sidebar(s: &State, k: &Kit) -> El {
    let nav = |page: Page, key: &str, title: &str, icon: &'static rune::Icon| {
        SidebarRow::new(title)
            .icon(icon)
            .selected(s.page == page || (page == Page::Home && s.page == Page::Configure))
            .height(36.0)
            .on_select(move |s: &mut State, _| s.navigate(page))
            .build(k)
            .key(key)
    };
    let mut sessions = SidebarSection::new("SESSÕES ABERTAS").count(s.workspace.len());
    for entry in &s.workspace {
        let key = entry.key;
        let active = s.active_workspace == Some(key);
        let (status, detail) = if active {
            if let Some((done, total)) = s.real_progress {
                ("Analisando", format!("Analisando {done} de {total}"))
            } else if let Some(error) = &s.bridge_error {
                ("Erro", format!("Erro · {error}"))
            } else if s.real_result.is_some() {
                ("Concluída", "Análise concluída".to_string())
            } else {
                (entry.status(), entry.status_detail().to_string())
            }
        } else {
            (entry.status(), entry.status_detail().to_string())
        };
        sessions = sessions.row(
            SidebarRow::new(&entry.title)
                .icon(&icons::BRANCH)
                .subtitle(status)
                .selected(active && s.page == Page::Review)
                .height(48.0)
                .on_select(move |s: &mut State, _| s.activate_review(key))
                .trailing(
                    row()
                        .key(format!("session-actions-{key}"))
                        .role("group")
                        .label("Ações da sessão")
                        .focus_group()
                        .child(
                            IconButton::new(k.glyph.close)
                                .variant(IconButtonVariant::GhostMuted)
                                .control_size(ControlSize::Small)
                                .label(format!("Fechar sessão {}", entry.title))
                                .build(k)
                                .key(format!("close-session-{key}"))
                                .on_press(move |s: &mut State| s.close_review(key)),
                        ),
                )
                .build(k)
                .key(format!("session-{key}"))
                .help(format!("{} — {detail}", entry.title)),
        );
    }
    if s.workspace.is_empty() {
        sessions = sessions.row(
            ui::label("Importe uma partida para começar", 12.0, k.c.text_muted)
                .wrap()
                .w_full()
                .padding(12.0),
        );
    }
    Sidebar::new()
        .label("Navegação principal")
        .row_gap(4.0)
        .header(
            row()
                .window_drag()
                .center_y()
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .px(12.0)
                        .py(12.0)
                        .gap(4.0)
                        .child(ui::label("EngineRoom", 19.0, k.c.text_base))
                        .child(ui::label("REVISÃO DE XADREZ", 12.0, k.c.text_muted)),
                )
                .child(toggle(true, k)),
        )
        .section(
            SidebarSection::new("ESPAÇO DE TRABALHO")
                .row(nav(Page::Home, "home-nav", "Nova revisão", &icons::PLUS))
                .row(nav(
                    Page::History,
                    "history-nav",
                    "Histórico",
                    &icons::FOLDER,
                )),
        )
        .section(sessions)
        .footer(
            SidebarRow::new("Configurações")
                .icon(&icons::SETTINGS_GEAR)
                .selected(s.page == Page::Settings)
                .height(36.0)
                .on_select(|s: &mut State, _| s.open_settings())
                .build(k)
                .key("settings-nav"),
        )
        .footer(
            col()
                .gap(5.0)
                .px(12.0)
                .py(8.0)
                .child(ui::label("Uma análise ativa por vez", 12.0, k.c.text_muted))
                .child(ui::label("EngineRoom · local", 12.0, k.c.text_muted)),
        )
        .build(k)
}
fn rail_button(icon: &'static rune::Icon, label: &str, selected: bool, k: &Kit) -> El {
    IconButton::new(icon)
        .variant(if selected {
            IconButtonVariant::Neutral
        } else {
            IconButtonVariant::GhostMuted
        })
        .control_size(ControlSize::Large)
        .label(label)
        .help(label)
        .build(k)
        .checked(selected)
}

fn toggle(open: bool, k: &Kit) -> El {
    let label = if open {
        "Recolher barra lateral"
    } else {
        "Abrir barra lateral"
    };
    IconButton::new(&SIDEBAR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Large)
        .label(label)
        .help(label)
        .shortcut(if cfg!(target_os = "macos") {
            "⌘B"
        } else {
            "Ctrl+B"
        })
        .expanded(open)
        .build(k)
        .key("sidebar-toggle")
        .on_press(|s: &mut State| s.sidebar.toggle())
}
pub fn view(s: &State, cx: &mut Cx<State>, main: El, trailing: Option<El>, k: &Kit) -> El {
    let open = s.sidebar.is_open();
    let mut shell = row().key("shell").w_full().h_full().fill(k.c.bg_deep);
    if open {
        let grip = el()
            .key("sidebar-grip")
            .role("separator")
            .label("Redimensionar barra lateral")
            .range_value(
                s.sidebar.width() as f64,
                s.sidebar.min_width() as f64,
                s.sidebar.max_width() as f64,
            )
            .range_step(8.0)
            .w(10.0)
            .h_full()
            .cursor(Cursor::ResizeColumn)
            .fill(Color::TRANSPARENT)
            .on_drag(|s: &mut State, d: &mut Drag, tx| match d.phase {
                Phase::Down => d.data = s.sidebar.width(),
                Phase::Move => {
                    tx.instant();
                    s.sidebar.set_width(d.data + d.dx);
                }
                Phase::Up => {
                    if d.cancelled() {
                        s.sidebar.set_width(d.data);
                    }
                }
            })
            .on_double_press(|s: &mut State| s.sidebar.reset_width())
            .on_step(|s: &mut State, steps, _| {
                s.sidebar.set_width(s.sidebar.width() + steps as f32 * 8.0)
            });
        shell = shell.child(
            row()
                .key("sidebar-pane")
                .w(s.sidebar.width())
                .h_full()
                .child(
                    col()
                        .grow(1.0)
                        .min_w(0.0)
                        .h_full()
                        .pt(if cfg!(target_os = "macos") && cx.titlebar_inset() > 0.0 {
                            40.0
                        } else {
                            0.0
                        })
                        .child(sidebar(s, k)),
                )
                .child(grip),
        );
    } else {
        shell = shell.child(
            col()
                .key("sidebar-rail")
                .w(36.0)
                .h_full()
                .pt(if cfg!(target_os = "macos") && cx.titlebar_inset() > 0.0 {
                    40.0
                } else {
                    12.0
                })
                .px(4.0)
                .gap(8.0)
                .child(toggle(false, k))
                .child(
                    rail_button(
                        &icons::PLUS,
                        "Nova revisão",
                        matches!(s.page, Page::Home | Page::Configure),
                        k,
                    )
                    .key("rail-home")
                    .on_press(|s: &mut State| s.navigate(Page::Home)),
                )
                .child(
                    rail_button(&icons::FOLDER, "Histórico", s.page == Page::History, k)
                        .key("rail-history")
                        .on_press(|s: &mut State| s.navigate(Page::History)),
                )
                .children(
                    s.active_workspace
                        .or_else(|| s.workspace.last().map(|entry| entry.key))
                        .map(|key| {
                            rail_button(
                                &icons::BRANCH,
                                "Abrir última sessão",
                                s.page == Page::Review,
                                k,
                            )
                            .key("rail-session")
                            .on_press(move |s: &mut State| s.activate_review(key))
                        }),
                )
                .child(
                    rail_button(
                        &icons::SETTINGS_GEAR,
                        "Configurações",
                        s.page == Page::Settings,
                        k,
                    )
                    .key("rail-settings")
                    .on_press(|s: &mut State| s.open_settings()),
                ),
        );
    }
    shell
        .child(
            col()
                .key("main-column")
                .grow(1.0)
                .min_w(0.0)
                .h_full()
                .px(8.0)
                .pt(if cfg!(target_os = "macos") && cx.titlebar_inset() > 0.0 {
                    40.0
                } else {
                    8.0
                })
                .pb(8.0)
                .child(
                    col()
                        .key("main-pane")
                        .role("main")
                        .grow(1.0)
                        .min_h(0.0)
                        .min_w(0.0)
                        .fill(k.c.page)
                        .radius(k.radius.lg)
                        .clip_rounded(k.radius.lg)
                        .child(main.grow(1.0).min_h(0.0)),
                ),
        )
        .children(trailing)
}

#[cfg(test)]
mod tests {
    use crate::state::{Page, State};
    use rune::{Env, Runtime};

    fn tap(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }

    #[test]
    fn collapsed_rail_navigates_and_returns_to_active_session() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (720.0, 560.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = State::fixture();
        rt.state.sidebar.set_open(false);
        rt.invalidate();
        rt.frame(0.0);
        tap(&mut rt, "rail-history");
        assert_eq!(rt.state.page, Page::History);
        tap(&mut rt, "rail-settings");
        assert_eq!(rt.state.page, Page::Settings);
        tap(&mut rt, "rail-home");
        assert_eq!(rt.state.page, Page::Home);
        tap(&mut rt, "rail-session");
        assert_eq!(rt.state.page, Page::Review);
        assert!(!rt.state.sidebar.is_open());
    }
}
