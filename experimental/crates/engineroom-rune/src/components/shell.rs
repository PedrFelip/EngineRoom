//! Native Rune desktop shell with a resizable sidebar.
use super::{icons, ui};
use crate::{
    session_style::{
        self, DragItem, FolderState, InsertSide, SessionColor, SessionDrag, SessionDropTarget,
        SessionFolder,
    },
    state::{Page, RenderStatus, State, WorkspaceReview},
};
use rune::{Color, Cursor, Cx, Drag, El, Phase, col, el, icon, row};
use rune_kit::{
    Button, ButtonVariant, ControlSize, Dialog, IconButton, IconButtonVariant, Kit, MenuItem,
    TextInput, context_menu,
    shell::{Sidebar, SidebarRow, SidebarSection},
};
use std::sync::{Arc, LazyLock};

const SESSIONS_TITLE: &str = "SESSÕES ABERTAS";
const LOGO_WIDTH: f32 = 116.0;
const LOGO_HEIGHT: f32 = LOGO_WIDTH * 601.0 / 854.0;
static DARK_LOGO: LazyLock<rune::Image> = LazyLock::new(|| {
    decode_logo(include_bytes!(
        "../../../../../docs/brand/engineroom-logo.svg"
    ))
});
static LIGHT_LOGO: LazyLock<rune::Image> = LazyLock::new(|| {
    decode_logo(include_bytes!(
        "../../../../../docs/brand/engineroom-logo-light.svg"
    ))
});

fn decode_logo(bytes: &'static [u8]) -> rune::Image {
    let cache = rune::ImageCache::new(Arc::new(|| {}));
    let image = cache.svg(bytes, LOGO_WIDTH, 3.0);
    // wait() publishes the pixels before returning, so the first headless frame has the logo
    // without depending on a cache wake.
    image.wait().expect("embedded EngineRoom logo must decode");
    image
}

/// The sidebar's context menu, mounted into the root layer stack by the caller. Every action
/// clears the transient menu state; the kit restores focus to the trigger on close.
fn session_menu(
    key: u64,
    current: Option<&SessionFolder>,
    trigger: El,
    s: &State,
    k: &Kit,
    layers: &mut Vec<El>,
) -> El {
    let point = s
        .session_menu
        .filter(|(target, _)| *target == key)
        .map(|(_, press)| press);
    let menu = context_menu(format!("session-menu-{key}"), trigger, point);
    if point.is_none() {
        return menu.build(k, layers);
    }
    let current = current.map(|folder| folder.id.as_str());
    let mut menu = menu.group_label("MOVER PARA PASTA");
    menu = menu.item(
        MenuItem::new("Sem pasta")
            .lead(session_style::marker(SessionColor::None, k))
            .checked(current.is_none())
            .on_select(move |s: &mut State| s.move_session(key, None)),
    );
    for folder in &s.preferences.sessions.folders {
        let id = folder.id.clone();
        menu = menu.item(
            MenuItem::new(&folder.name)
                .lead(session_style::marker(folder.color, k))
                .checked(current == Some(folder.id.as_str()))
                .on_select(move |s: &mut State| s.move_session(key, Some(id.clone()))),
        );
    }
    menu = menu
        .separator()
        .item(
            MenuItem::new("Nova pasta…")
                .icon(&icons::PLUS)
                .on_select(move |s: &mut State| s.edit_folder(None, Some(key))),
        )
        .separator()
        .item(
            MenuItem::new("Fechar sessão")
                .icon(&icons::CLOSE)
                .danger(true)
                .on_select(move |s: &mut State| s.close_review(key)),
        )
        .on_close(|s: &mut State| s.close_session_menu());
    menu.build(k, layers)
}

fn drag_session(s: &mut State, item: DragItem, drag: &mut Drag, tx: &mut rune::Tx) {
    match drag.phase {
        Phase::Move | Phase::Up
            if !drag.cancelled() && (s.session_drag.is_some() || drag.dx.hypot(drag.dy) >= 6.0) =>
        {
            s.session_menu = None;
            s.folder_menu = None;
            let active = s.session_drag.get_or_insert(SessionDrag {
                item: item.clone(),
                point: drag.window_point(),
                target: None,
            });
            active.point = drag.window_point();
            let drop = drag.phase == Phase::Up;
            let [x, y] = drag.window_point();
            tx.element_at_in("sidebar-list", x, y, move |s: &mut State, picked, _| {
                if !s
                    .session_drag
                    .as_ref()
                    .is_some_and(|drag| drag.item == item)
                {
                    return;
                }
                let target = picked.and_then(|picked| s.session_drop_target(&item, picked.path()));
                if drop {
                    s.session_drag = None;
                    if let Some(target) = target {
                        s.apply_session_drop(item, target);
                    }
                } else if let Some(drag) = &mut s.session_drag {
                    drag.target = target;
                }
            });
        }
        Phase::Down | Phase::Up => s.session_drag = None,
        Phase::Move => {}
    }
}

fn reorder_indicator(s: &State, item: &DragItem, k: &Kit) -> Option<El> {
    let Some(SessionDropTarget::Reorder(target, side)) = s.session_drag.as_ref()?.target.as_ref()
    else {
        return None;
    };
    if item != target {
        return None;
    }
    let (top, bottom) = match side {
        InsertSide::Before => (Some(0.0), None),
        InsertSide::After => (None, Some(0.0)),
    };
    Some(
        el().key("session-reorder-indicator")
            .pin(Some(0.0), top, Some(0.0), bottom)
            .h(2.0)
            .fill(k.c.bg_accent)
            .a11y_hidden()
            .inert(),
    )
}

fn session_row(
    s: &State,
    entry: &WorkspaceReview,
    status: RenderStatus,
    folder: Option<&SessionFolder>,
    k: &Kit,
    layers: &mut Vec<El>,
) -> El {
    debug_assert_eq!(status.label(), entry.status());
    debug_assert_eq!(status.detail(), entry.status_detail());
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
            (status.label(), status.detail().to_string())
        }
    } else {
        (status.label(), status.detail().to_string())
    };
    let folder_name = folder.map_or("Sem pasta", |folder| folder.name.as_str());
    let item = SidebarRow::new(&entry.title)
        .subtitle(status)
        .subtitle_color(if active && s.real_progress.is_some() {
            Color::TRANSPARENT
        } else {
            k.c.text_faint
        })
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
                    ui::icon_feedback(
                        IconButton::new(&icons::CLOSE)
                            .variant(IconButtonVariant::GhostMuted)
                            .control_size(ControlSize::Small)
                            .label(format!("Fechar sessão {}", entry.title))
                            .build(k),
                        k,
                    )
                    .group_opacity(0.0, 1.0)
                    .key(format!("close-session-{key}"))
                    .on_press(move |s: &mut State| s.close_review(key)),
                ),
        );
    let item = if folder.is_none() {
        item.icon(&icons::SESSION)
    } else {
        item
    };
    let drag_item = DragItem::Session(key);
    let dragging = s
        .session_drag
        .as_ref()
        .is_some_and(|drag| drag.item == drag_item);
    let row = item
        .build(k)
        .key(format!("session-{key}"))
        .hover_group_focus()
        .cursor(if dragging {
            Cursor::Grabbing
        } else {
            Cursor::Grab
        })
        .opacity(if dragging { 0.55 } else { 1.0 })
        .label(format!("{}, {status}, Pasta: {}", entry.title, folder_name))
        .help(format!(
            "{} — {detail} · {} · Arraste para mover · Shift+F10 para opções",
            entry.title, folder_name
        ))
        .on_secondary_press(move |s: &mut State, press, _| s.open_session_menu(key, press))
        .on_drag(move |s: &mut State, drag, tx| drag_session(s, DragItem::Session(key), drag, tx))
        .children(reorder_indicator(s, &drag_item, k))
        .children((active && s.real_progress.is_some()).then(|| {
            ui::analysis_label(status, k.ty.caption(), s, k)
                .absolute(if folder.is_some() { 8.0 } else { 30.0 }, 25.0)
                .inert()
                .a11y_hidden()
        }));
    session_menu(key, folder, row, s, k, layers)
}

fn sidebar(s: &State, k: &Kit, layers: &mut Vec<El>) -> El {
    let logo = if k.dark { &DARK_LOGO } else { &LIGHT_LOGO };
    let nav = |page: Page, key: &str, title: &str, icon: &'static rune::Icon| {
        SidebarRow::new(title)
            .icon(icon)
            .selected(s.page == page || (page == Page::Home && s.page == Page::Configure))
            .height(36.0)
            .on_select(move |s: &mut State, _| s.navigate(page))
            .build(k)
            .key(key)
    };
    let groups = s.session_groups();
    let mut shell = Sidebar::new()
        .label("Navegação principal")
        .row_gap(4.0)
        .header(
            row()
                .window_drag()
                .center_y()
                .child(
                    col().grow(1.0).min_w(0.0).px(12.0).py(12.0).child(
                        el().key("sidebar-logo")
                            .role("img")
                            .label("EngineRoom")
                            .w(LOGO_WIDTH)
                            .h(LOGO_HEIGHT)
                            .pixels_fit(logo.pixels(), rune::SurfaceFit::Fit),
                    ),
                )
                .child(toggle(true, k)),
        )
        .child(nav(Page::Home, "home-nav", "Nova revisão", &icons::PLUS))
        .child(nav(
            Page::History,
            "history-nav",
            "Histórico",
            &icons::HISTORY,
        ));
    let empty_hint = || {
        ui::label("Importe uma partida para começar", 12.0, k.c.text_muted)
            .wrap()
            .w_full()
            .padding(12.0)
    };
    shell = shell.section(
        SidebarSection::new(SESSIONS_TITLE)
            .count(s.workspace.len())
            .action(
                row().focus_group().child(
                    IconButton::new(&icons::PLUS)
                        .variant(IconButtonVariant::GhostMuted)
                        .control_size(ControlSize::Small)
                        .label("Nova pasta")
                        .build(k)
                        .key("new-folder")
                        .on_press(|s: &mut State| s.edit_folder(None, None)),
                ),
            ),
    );
    for group in groups {
        if let Some(folder) = group.folder {
            let collapsed = s
                .preferences
                .sessions
                .collapsed_folders
                .contains(&folder.id);
            let id = folder.id.clone();
            let menu_id = id.clone();
            let drag_item = DragItem::Folder(id.clone());
            let is_source = s
                .session_drag
                .as_ref()
                .is_some_and(|drag| drag.item == drag_item);
            let drag_source = drag_item.clone();
            let header = row()
                .w_full()
                .h(38.0)
                .px(8.0)
                .gap(8.0)
                .center_y()
                .radius(k.radius.md)
                .hover_group_focus()
                .child(
                    icon(
                        if collapsed {
                            k.glyph.caret_right
                        } else {
                            k.glyph.chevron_down
                        },
                        12.0,
                    )
                    .color(k.c.icon_faint)
                    .a11y_hidden(),
                )
                .child(session_style::folder_marker(
                    folder.color,
                    if collapsed {
                        FolderState::Collapsed
                    } else {
                        FolderState::Open
                    },
                    k,
                ))
                .child(
                    ui::label(&folder.name, 13.0, k.c.text_base)
                        .grow(1.0)
                        .min_w(0.0)
                        .truncate()
                        .a11y_hidden(),
                )
                .child(
                    ui::label(group.entries.len().to_string(), 11.0, k.c.text_faint).a11y_hidden(),
                );
            let header = k
                .interactive(header)
                .key(format!("folder-toggle-{}", folder.id))
                .label(&folder.name)
                .description(format!("{} sessões", group.entries.len()))
                .help(format!(
                    "{} · Arraste para reordenar · Botão direito ou Shift+F10 para opções da pasta",
                    folder.name
                ))
                .cursor(if is_source {
                    Cursor::Grabbing
                } else {
                    Cursor::Grab
                })
                .opacity(if is_source { 0.55 } else { 1.0 })
                .on_drag(move |s: &mut State, drag, tx| {
                    drag_session(s, drag_source.clone(), drag, tx);
                })
                .expanded(!collapsed)
                .focusable(true)
                .focus_highlight(k.c.layer_02)
                .on_press_tx(move |s: &mut State, tx| {
                    s.toggle_folder(id.clone());
                    tx.focus(format!("folder-toggle-{id}"), true);
                })
                .on_secondary_press(move |s: &mut State, press, _| {
                    s.open_folder_menu(menu_id.clone(), press);
                })
                .role("button");
            let point = s
                .folder_menu
                .as_ref()
                .filter(|(id, _)| id == &folder.id)
                .map(|(_, point)| *point);
            let menu = context_menu(format!("folder-menu-{}", folder.id), header, point);
            let menu =
                if point.is_some() {
                    let edit_id = folder.id.clone();
                    let delete_id = folder.id.clone();
                    menu.min_width(240.0)
                        .item(MenuItem::new("Editar pasta…").icon(&icons::GEAR).on_select(
                            move |s: &mut State| s.edit_folder(Some(edit_id.clone()), None),
                        ))
                        .separator()
                        .item(
                            MenuItem::new("Excluir pasta")
                                .icon(&icons::TRASH)
                                .danger(true)
                                .description("Mantém as sessões abertas")
                                .on_select(move |s: &mut State| s.delete_folder(&delete_id)),
                        )
                        .on_close(|s: &mut State| s.folder_menu = None)
                } else {
                    menu
                };
            let header = menu.build(k, layers).focus_group();
            let hovered = s.session_drag.as_ref().is_some_and(|drag| {
                matches!(&drag.target, Some(SessionDropTarget::Folder(id)) if id == &folder.id)
            });
            let mut section = col()
                .radius(k.radius.md)
                .fill(if hovered {
                    k.alpha(k.c.bg_accent, 0.10)
                } else {
                    Color::TRANSPARENT
                })
                .border(
                    1.0,
                    if hovered {
                        k.c.bg_accent
                    } else {
                        Color::TRANSPARENT
                    },
                )
                .key(format!("session-folder-{}", folder.id))
                .role("group")
                .label(&folder.name)
                .w_full()
                .gap(2.0)
                .child(header);
            if !collapsed {
                let mut contents = col().w_full().min_w(0.0).pl(26.0).gap(2.0);
                if group.entries.is_empty() {
                    contents = contents.child(
                        ui::label("Nenhuma sessão", 11.0, k.c.text_faint)
                            .px(8.0)
                            .py(10.0),
                    );
                }
                for slot in group.entries {
                    let entry = slot
                        .get(&s.workspace)
                        .expect("group slot index comes from this workspace");
                    contents = contents.child(session_row(
                        s,
                        entry,
                        slot.status(),
                        Some(folder),
                        k,
                        layers,
                    ));
                }
                section = section.child(contents);
            }
            shell = shell.child(section.children(reorder_indicator(s, &drag_item, k)));
        } else {
            let dragging = s
                .session_drag
                .as_ref()
                .is_some_and(|drag| matches!(drag.item, DragItem::Session(_)));
            if group.entries.is_empty() && !s.preferences.sessions.folders.is_empty() && !dragging {
                continue;
            }
            let hovered = s
                .session_drag
                .as_ref()
                .is_some_and(|drag| drag.target == Some(SessionDropTarget::Unfiled));
            let mut rows = col()
                .key("session-unfiled")
                .role("group")
                .label("Sessões fora de pastas")
                .w_full()
                .gap(2.0)
                .radius(k.radius.md)
                .fill(if hovered {
                    k.alpha(k.c.bg_accent, 0.10)
                } else {
                    Color::TRANSPARENT
                })
                .border(
                    1.0,
                    if hovered {
                        k.c.bg_accent
                    } else {
                        Color::TRANSPARENT
                    },
                );
            if dragging {
                rows = rows.child(
                    ui::label(
                        "Solte aqui para deixar fora de pastas",
                        11.0,
                        k.c.text_muted,
                    )
                    .wrap()
                    .padding(10.0),
                );
            } else if group.entries.is_empty() {
                rows = rows.child(empty_hint());
            }
            for slot in group.entries {
                let entry = slot
                    .get(&s.workspace)
                    .expect("group slot index comes from this workspace");
                rows = rows.child(session_row(s, entry, slot.status(), None, k, layers));
            }
            shell = shell.child(rows);
        }
    }
    shell
        .footer(
            SidebarRow::new("Configurações")
                .icon(&icons::GEAR)
                .selected(s.page == Page::Settings)
                .height(36.0)
                .on_select(|s: &mut State, _| s.open_settings())
                .build(k)
                .key("settings-nav"),
        )
        .footer(col().gap(5.0).px(12.0).py(8.0))
        .build(k)
}
fn rail_button(icon: &'static rune::Icon, label: &str, selected: bool, k: &Kit) -> El {
    ui::icon_feedback(
        IconButton::new(icon)
            .variant(if selected {
                IconButtonVariant::Neutral
            } else {
                IconButtonVariant::GhostMuted
            })
            .control_size(ControlSize::Large)
            .label(label)
            .help(label)
            .build(k),
        k,
    )
    .checked(selected)
}

fn toggle(open: bool, k: &Kit) -> El {
    let label = if open {
        "Recolher barra lateral"
    } else {
        "Abrir barra lateral"
    };
    ui::icon_feedback(
        IconButton::new(if open {
            &icons::SIDEBAR_COLLAPSE
        } else {
            &icons::SIDEBAR_EXPAND
        })
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
        .build(k),
        k,
    )
    .key("sidebar-toggle")
    .on_press(|s: &mut State| s.sidebar.toggle())
}
pub fn view(
    s: &State,
    cx: &mut Cx<State>,
    main: El,
    trailing: Option<El>,
    k: &Kit,
    layers: &mut Vec<El>,
) -> El {
    if let Some(drag) = &s.session_drag
        && let Some((title, glyph)) = match &drag.item {
            DragItem::Session(key) => s
                .workspace
                .iter()
                .find(|entry| entry.key == *key)
                .map(|entry| (entry.title.as_str(), &icons::SESSION)),
            DragItem::Folder(id) => s
                .preferences
                .sessions
                .folders
                .iter()
                .find(|folder| folder.id == *id)
                .map(|folder| (folder.name.as_str(), &rune_kit::glyphs::FOLDER)),
        }
    {
        layers.push(
            row()
                .absolute(drag.point[0] + 14.0, drag.point[1] + 14.0)
                .max_w(220.0)
                .px(10.0)
                .py(8.0)
                .gap(6.0)
                .center_y()
                .fill(k.c.layer_02)
                .border(1.0, k.c.border_base)
                .radius(k.radius.md)
                .child(icon(glyph, 14.0).color(k.c.icon_muted))
                .child(ui::label(title, 12.0, k.c.text_base).min_w(0.0).truncate())
                .key("session-drag-preview")
                .a11y_hidden()
                .inert(),
        );
    }
    folder_dialog(s, k, layers);
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
                        .child(sidebar(s, k, layers)),
                )
                .child(grip),
        );
    } else {
        shell = shell.child(
            col()
                .key("sidebar-rail")
                .pb(12.0)
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
                    rail_button(&icons::HISTORY, "Histórico", s.page == Page::History, k)
                        .key("rail-history")
                        .on_press(|s: &mut State| s.navigate(Page::History)),
                )
                .children(
                    s.active_workspace
                        .or_else(|| s.workspace.last().map(|entry| entry.key))
                        .map(|key| {
                            rail_button(
                                &icons::SESSION,
                                "Abrir última sessão",
                                s.page == Page::Review,
                                k,
                            )
                            .key("rail-session")
                            .on_press(move |s: &mut State| s.activate_review(key))
                        }),
                )
                .child(el().grow(1.0).min_h(0.0))
                .child(
                    rail_button(&icons::GEAR, "Configurações", s.page == Page::Settings, k)
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

fn folder_dialog(s: &State, k: &Kit, layers: &mut Vec<El>) {
    let Some(editor) = &s.folder_editor else {
        return;
    };
    let trimmed = s.folder_name.text.trim();
    let valid = !trimmed.is_empty() && trimmed.chars().count() <= 80;
    let mut content = col()
        .gap(16.0)
        .padding(20.0)
        .child(ui::label("Nome", 12.0, k.c.text_muted))
        .child(
            TextInput::new(&s.folder_name)
                .label("Nome da pasta")
                .placeholder("Ex.: Aberturas")
                .width(None)
                .on_edit(|s: &mut State| &mut s.folder_name)
                .build(k)
                .key("folder-name"),
        );
    let mut colors = row()
        .gap(8.0)
        .role("radiogroup")
        .label("Cor da pasta")
        .focus_group();
    let ui_theme = crate::theme::kit_theme(k);
    for color in SessionColor::ALL {
        let selected = color == editor.color;
        let swatch = if color == SessionColor::None {
            k.c.layer_03
        } else {
            crate::theme::session_swatch(color.slug(), ui_theme)
        };
        colors = colors.child(
            k.interactive(
                el().size(32.0, 32.0)
                    .center()
                    .radius(16.0)
                    .border(if selected { 1.5 } else { 0.0 }, k.c.text_base)
                    .child(
                        el().size(22.0, 22.0)
                            .radius(11.0)
                            .fill(swatch)
                            .center()
                            .children(
                                (color == SessionColor::None)
                                    .then(|| icon(k.glyph.dash, 12.0).color(k.c.text_muted)),
                            ),
                    ),
            )
            .key(format!("folder-color-{}", color.slug()))
            .label(color.label())
            .help(color.label())
            .focusable(true)
            .on_press(move |s: &mut State| {
                if let Some(editor) = &mut s.folder_editor {
                    editor.color = color;
                }
            })
            .role("radio")
            .focus_selected(selected)
            .checked(selected),
        );
    }
    content = content.child(
        col()
            .gap(8.0)
            .child(
                row()
                    .gap(8.0)
                    .center_y()
                    .child(ui::label("Cor", 12.0, k.c.text_muted))
                    .child(ui::label(editor.color.label(), 11.0, k.c.text_faint)),
            )
            .child(colors),
    );
    content = content.child(
        row()
            .w_full()
            .gap(8.0)
            .child(el().grow(1.0))
            .child(
                Button::new("Cancelar")
                    .variant(ButtonVariant::Ghost)
                    .on_press(|s: &mut State| s.folder_editor = None)
                    .build(k),
            )
            .child(
                Button::new("Salvar")
                    .variant(ButtonVariant::Submit)
                    .disabled(!valid)
                    .on_press(|s: &mut State| s.save_folder())
                    .build(k)
                    .key("save-folder"),
            ),
    );
    Dialog::new(
        "folder-editor",
        if editor.id.is_some() {
            "Editar pasta"
        } else {
            "Nova pasta"
        },
    )
    .open(true)
    .width(400.0)
    .fit()
    .on_close(|s: &mut State| s.folder_editor = None)
    .child(content)
    .build(k, layers);
}

#[cfg(test)]
mod tests {
    use crate::{
        session_style::SessionColor,
        state::{Page, State},
    };
    use rune::{Env, Key, KeyEvent, Mods, Runtime};

    #[test]
    fn embedded_logos_decode_before_the_first_frame() {
        for (bytes, name) in [
            (
                &include_bytes!("../../../../../docs/brand/engineroom-logo.svg")[..],
                "dark",
            ),
            (
                &include_bytes!("../../../../../docs/brand/engineroom-logo-light.svg")[..],
                "light",
            ),
        ] {
            let logo = super::decode_logo(bytes);
            let [width, height] = logo.wait().expect("embedded logo must decode");
            assert!(width > 0 && height > 0, "{name} logo has pixels");
            assert!(logo.is_ready(), "{name} logo is ready synchronously");
        }
    }

    #[test]
    #[ignore = "opt-in sidebar benchmark"]
    fn benchmark_sidebar_presentation() {
        use crate::session_style::{DragItem, SessionFolder};
        use std::{hint::black_box, time::Instant};

        fn nanos_per_call(iterations: u32, mut run: impl FnMut()) -> u128 {
            run();
            let mut samples = [0; 7];
            for sample in &mut samples {
                let start = Instant::now();
                for _ in 0..iterations {
                    run();
                }
                *sample = start.elapsed().as_nanos() / u128::from(iterations);
            }
            samples.sort_unstable();
            samples[3]
        }

        println!("sessions,folders,group_ns,drop_target_ns,build_sidebar_ns");
        let kit = crate::theme::kit(crate::state::Appearance::Dark);
        for (sessions, folders) in [(12, 4), (128, 16), (512, 64)] {
            let mut state = State::fixture();
            let config = state.workspace[0].config.clone();
            state.preferences.sessions.folders = (0..folders)
                .map(|i| SessionFolder {
                    id: format!("bench-{i}"),
                    name: format!("Pasta {i}"),
                    color: SessionColor::Blue,
                })
                .collect();
            while state.workspace.len() < sessions {
                state.register_review(config.clone());
            }
            for (i, entry) in state.workspace.iter_mut().enumerate() {
                entry.folder_id = Some(format!("bench-{}", i % folders));
            }
            for entry in state.workspace.iter().rev() {
                state
                    .session_order
                    .entry(entry.folder_id.clone().unwrap())
                    .or_default()
                    .push(entry.key);
            }
            let source = DragItem::Session(state.workspace[0].key);
            let target = format!(
                "session-folder-bench-0/session-{}",
                state.workspace[folders].key
            );
            assert!(state.session_drop_target(&source, &target).is_some());
            let grouping = nanos_per_call(200, || {
                black_box(black_box(&state).session_groups());
            });
            let pick = nanos_per_call(200, || {
                black_box(black_box(&state).session_drop_target(&source, &target));
            });
            let build = nanos_per_call(8, || {
                black_box(super::sidebar(black_box(&state), &kit, &mut Vec::new()));
            });
            println!("{sessions},{folders},{grouping},{pick},{build}");
        }
    }

    fn tap(rt: &mut Runtime<State>, key: &str) {
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
    }
    fn choose_folder_action(rt: &mut Runtime<State>, id: &str, action: &str) {
        let (x, y) = rt
            .find(&format!("folder-toggle-{id}"))
            .expect("folder header");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(rt);
        let (x, y) = rt
            .elements()
            .into_iter()
            .find(|e| e.label.as_deref() == Some(action))
            .map(|e| (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0))
            .expect("edit menu item");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(rt);
    }
    fn settle(rt: &mut Runtime<State>) {
        for _ in 0..40 {
            rt.frame(1.0 / 60.0);
        }
    }
    fn key(rt: &mut Runtime<State>, key: Key, shift: bool) {
        rt.key(KeyEvent {
            key,
            mods: Mods {
                shift,
                ..Mods::default()
            },
        });
        settle(rt);
    }
    fn focused(rt: &Runtime<State>) -> Option<String> {
        rt.elements()
            .into_iter()
            .find(|e| e.focused)
            .and_then(|e| e.key)
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

    #[test]
    fn session_context_menu_moves_and_closes_from_the_pointer() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = State::fixture();
        let key = rt.state.workspace[0].key;
        rt.state.edit_folder(None, None);
        rt.state.folder_name.replace_text("Aberturas");
        rt.state.save_folder();
        settle(&mut rt);
        let (x, y) = rt.find(&format!("session-{key}")).expect("session row");
        rt.secondary_down(x, y);
        rt.secondary_up(x, y);
        settle(&mut rt);
        assert_eq!(rt.state.session_menu.map(|(target, _)| target), Some(key));
        assert!(
            rt.elements()
                .iter()
                .any(|e| e.key.as_deref() == Some(&format!("session-menu-{key}-menu"))),
            "the menu surface is mounted"
        );

        let (x, y) = rt
            .elements()
            .into_iter()
            .find(|e| {
                e.label.as_deref() == Some("Aberturas")
                    && e.key
                        .as_deref()
                        .is_some_and(|key| key.starts_with("session-menu-"))
            })
            .map(|e| (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0))
            .expect("folder menu item");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert_eq!(rt.state.session_folder(key).unwrap().name, "Aberturas");
        assert!(
            rt.elements().iter().any(|e| {
                e.key.as_deref() == Some(&format!("session-{key}"))
                    && e.label
                        .as_deref()
                        .is_some_and(|label| label.contains("Pasta: Aberturas"))
            }),
            "the folder is named on the session row"
        );
        assert_eq!(
            rt.state.session_menu, None,
            "moving to a folder closes the menu"
        );
    }

    #[test]
    fn session_context_menu_opens_from_the_keyboard_and_returns_focus() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = State::fixture();
        rt.set_tab_visits_controls(true);
        settle(&mut rt);
        let mut in_sidebar = false;
        for _ in 0..12 {
            key(&mut rt, Key::Tab, false);
            if focused(&rt).as_deref() == Some("home-nav") {
                in_sidebar = true;
                break;
            }
        }
        assert!(in_sidebar, "the sidebar list is reachable with Tab");
        // The list uses focus-follows: arrows step through rows inside it.
        let mut focused_row = false;
        for _ in 0..8 {
            key(&mut rt, Key::Down, false);
            if focused(&rt).as_deref() == Some("session-1") {
                focused_row = true;
                break;
            }
        }
        assert!(focused_row, "the session row is reachable with arrow keys");

        key(&mut rt, Key::Function(10), true);
        assert_eq!(rt.state.session_menu.map(|(target, _)| target), Some(1));
        key(&mut rt, Key::Escape, false);
        assert_eq!(rt.state.session_menu, None);
        assert_eq!(focused(&rt).as_deref(), Some("session-1"), "focus returns");

        key(&mut rt, Key::ContextMenu, false);
        assert!(rt.state.session_menu.is_some(), "the Menu key opens it too");
        key(&mut rt, Key::Escape, false);
        assert_eq!(rt.state.session_menu, None);
    }
    #[test]
    fn folder_editor_creates_renames_collapses_and_deletes_without_closing_sessions() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = State::fixture();
        settle(&mut rt);
        tap(&mut rt, "new-folder");
        settle(&mut rt);
        assert!(rt.find("folder-name").is_some());
        rt.state.folder_name.replace_text("Aberturas");
        rt.invalidate();
        settle(&mut rt);
        tap(&mut rt, "folder-color-green");
        tap(&mut rt, "save-folder");
        settle(&mut rt);
        let id = rt.state.preferences.sessions.folders[0].id.clone();
        let key = rt.state.workspace[0].key;
        rt.state.move_session(key, Some(id.clone()));
        rt.state.toggle_folder(id.clone());
        rt.invalidate();
        settle(&mut rt);
        assert!(rt.find(&format!("session-{key}")).is_none());
        choose_folder_action(&mut rt, &id, "Editar pasta…");
        settle(&mut rt);
        rt.state.folder_name.replace_text("Estudos");
        rt.invalidate();
        settle(&mut rt);
        tap(&mut rt, "save-folder");
        settle(&mut rt);
        assert_eq!(rt.state.session_folder(key).unwrap().name, "Estudos");
        assert_eq!(
            rt.state.session_folder(key).unwrap().color,
            crate::session_style::SessionColor::Green
        );
        choose_folder_action(&mut rt, &id, "Excluir pasta");
        settle(&mut rt);
        assert!(rt.state.preferences.sessions.folders.is_empty());
        assert!(rt.find(&format!("session-{key}")).is_some());
    }
    #[test]
    fn folder_context_menu_supports_keyboard_and_returns_focus() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = State::fixture();
        rt.set_tab_visits_controls(true);
        let session = rt.state.workspace[0].key;
        crate::fixtures::file_session(
            &mut rt.state,
            session,
            crate::session_style::SessionColor::Blue,
        );
        settle(&mut rt);
        tap(&mut rt, "folder-toggle-fixture-blue");
        settle(&mut rt);
        assert!(
            rt.state
                .preferences
                .sessions
                .collapsed_folders
                .contains("fixture-blue")
        );
        assert_eq!(
            focused(&rt).as_deref(),
            Some("folder-toggle-fixture-blue"),
            "folder keeps keyboard focus after collapsing"
        );
        key(&mut rt, Key::Function(10), true);
        assert_eq!(
            rt.state.folder_menu.as_ref().map(|(id, _)| id.as_str()),
            Some("fixture-blue")
        );
        key(&mut rt, Key::Escape, false);
        assert!(rt.state.folder_menu.is_none());
        assert_eq!(focused(&rt).as_deref(), Some("folder-toggle-fixture-blue"));
        key(&mut rt, Key::Enter, false);
        assert!(
            !rt.state
                .preferences
                .sessions
                .collapsed_folders
                .contains("fixture-blue")
        );
        assert!(rt.find(&format!("session-{session}")).is_some());
    }
    fn begin_drag(rt: &mut Runtime<State>, source: &str) {
        let (x, y) = rt
            .find(source)
            .unwrap_or_else(|| panic!("drag source {source} not found"));
        rt.pointer_down(x, y);
        rt.pointer_move(x + 10.0, y);
        settle(rt);
        assert!(rt.state.session_drag.is_some());
    }

    fn drop_on(rt: &mut Runtime<State>, target: &str) {
        let (x, y) = rt.find(target).expect("drop target");
        rt.pointer_move(x, y);
        settle(rt);
        assert!(rt.state.session_drag.as_ref().unwrap().target.is_some());
        rt.pointer_up(x, y);
        settle(rt);
        assert!(rt.state.session_drag.is_none());
    }

    #[test]
    fn dragging_reorders_folders_and_their_sessions_in_both_directions() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::fixtures::workspace_scene();
        let first = rt.state.workspace[0].key;
        let second = rt.state.workspace[1].key;
        crate::fixtures::file_session(&mut rt.state, first, SessionColor::Blue);
        crate::fixtures::file_session(&mut rt.state, second, SessionColor::Amber);
        rt.state.move_session(second, Some("fixture-blue".into()));
        rt.state.toggle_folder("fixture-amber".into());
        let active = rt.state.active_workspace;
        let blue = "folder-toggle-fixture-blue";
        let amber = "folder-toggle-fixture-amber";
        let first_row = format!("session-{first}");
        let second_row = format!("session-{second}");
        settle(&mut rt);

        begin_drag(&mut rt, blue);
        let (x, y) = rt.find(amber).unwrap();
        rt.pointer_move(x, y);
        settle(&mut rt);
        assert!(rt.find("session-reorder-indicator").is_some());
        key(&mut rt, Key::Escape, false);
        rt.pointer_up(x, y);
        settle(&mut rt);
        assert!(rt.find(blue).unwrap().1 < rt.find(amber).unwrap().1);

        begin_drag(&mut rt, amber);
        drop_on(&mut rt, blue);
        assert!(rt.find(amber).unwrap().1 < rt.find(blue).unwrap().1);
        let serialized = serde_json::to_string(&rt.state.preferences).unwrap();
        let restored: crate::state::Preferences = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored.sessions.folders[0].id, "fixture-amber");
        assert!(
            restored
                .sessions
                .collapsed_folders
                .contains("fixture-amber")
        );
        assert!(rt.find(&first_row).is_some());

        begin_drag(&mut rt, amber);
        drop_on(&mut rt, &second_row);
        assert!(rt.find(blue).unwrap().1 < rt.find(amber).unwrap().1);

        begin_drag(&mut rt, &first_row);
        drop_on(&mut rt, &second_row);
        assert!(rt.find(&second_row).unwrap().1 < rt.find(&first_row).unwrap().1);
        assert_eq!(rt.state.active_workspace, active);
        assert_eq!(rt.state.workspace[0].key, first);
        assert_eq!(rt.state.workspace[1].key, second);

        begin_drag(&mut rt, &first_row);
        drop_on(&mut rt, &second_row);
        assert!(rt.find(&first_row).unwrap().1 < rt.find(&second_row).unwrap().1);
        assert_eq!(rt.state.active_workspace, active);
    }

    #[test]
    fn dragging_moves_between_folders_and_unfiled_without_switching_reviews() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::fixtures::workspace_scene();
        let first = rt.state.workspace[0].key;
        let second = rt.state.workspace[1].key;
        crate::fixtures::file_session(
            &mut rt.state,
            first,
            crate::session_style::SessionColor::Blue,
        );
        crate::fixtures::file_session(
            &mut rt.state,
            second,
            crate::session_style::SessionColor::Amber,
        );
        rt.state.toggle_folder("fixture-blue".into());
        let original: Vec<_> = rt.state.workspace.iter().map(|entry| entry.key).collect();
        let active = rt.state.active_workspace;
        settle(&mut rt);
        begin_drag(&mut rt, &format!("session-{second}"));
        drop_on(&mut rt, "folder-toggle-fixture-blue");
        assert_eq!(rt.state.session_folder(second).unwrap().id, "fixture-blue");
        assert!(
            !rt.state
                .preferences
                .sessions
                .collapsed_folders
                .contains("fixture-blue")
        );
        assert_eq!(
            rt.state.active_workspace, active,
            "dragging must not navigate or restart analysis"
        );
        begin_drag(&mut rt, &format!("session-{second}"));
        drop_on(&mut rt, "folder-toggle-fixture-amber");
        assert_eq!(rt.state.session_folder(second).unwrap().id, "fixture-amber");
        begin_drag(&mut rt, &format!("session-{second}"));
        drop_on(&mut rt, "session-unfiled");
        assert!(rt.state.session_folder(second).is_none());
        assert_eq!(
            rt.state
                .workspace
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>(),
            original
        );
    }

    #[test]
    fn dragging_cancels_on_escape_and_outside_drop_and_click_still_navigates() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::fixtures::workspace_scene();
        let session = rt.state.workspace[1].key;
        crate::fixtures::file_session(
            &mut rt.state,
            session,
            crate::session_style::SessionColor::Blue,
        );
        settle(&mut rt);
        begin_drag(&mut rt, &format!("session-{session}"));
        key(&mut rt, Key::Escape, false);
        assert!(rt.state.session_drag.is_none());
        assert_eq!(rt.state.session_folder(session).unwrap().id, "fixture-blue");
        begin_drag(&mut rt, &format!("session-{session}"));
        rt.pointer_move(1100.0, 100.0);
        rt.pointer_up(1100.0, 100.0);
        settle(&mut rt);
        assert!(rt.state.session_drag.is_none());
        assert_eq!(rt.state.session_folder(session).unwrap().id, "fixture-blue");
        tap(&mut rt, &format!("session-{session}"));
        settle(&mut rt);
        assert_eq!(rt.state.active_workspace, Some(session));
        assert!(rt.state.session_drag.is_none());
    }

    #[test]
    fn dragging_back_near_origin_updates_point_and_clears_previous_target() {
        let mut rt = rune::App::new(crate::spec()).headless_runtime(
            (1220.0, 900.0),
            Env {
                system_dark: true,
                scale: 1.0,
            },
        );
        rt.state = crate::fixtures::workspace_scene();
        let first = rt.state.workspace[0].key;
        let second = rt.state.workspace[1].key;
        crate::fixtures::file_session(&mut rt.state, first, SessionColor::Blue);
        crate::fixtures::file_session(&mut rt.state, second, SessionColor::Amber);
        settle(&mut rt);

        let source = format!("session-{second}");
        let (sx, sy) = rt.find(&source).expect("drag source");
        let (tx, ty) = rt
            .find("folder-toggle-fixture-blue")
            .expect("target folder");
        rt.pointer_down(sx, sy);
        rt.pointer_move(tx, ty);
        settle(&mut rt);
        assert!(rt.state.session_drag.as_ref().unwrap().target.is_some());

        // Returning within the 6px start hysteresis must still update the active drag.
        rt.pointer_move(sx + 2.0, sy + 1.0);
        settle(&mut rt);
        let active = rt.state.session_drag.as_ref().expect("drag remains active");
        assert_eq!(active.point, [sx + 2.0, sy + 1.0]);
        assert_eq!(active.target, None);

        rt.pointer_up(sx + 2.0, sy + 1.0);
        settle(&mut rt);
        assert!(rt.state.session_drag.is_none());
        assert_eq!(rt.state.session_folder(second).unwrap().id, "fixture-amber");
    }
}
