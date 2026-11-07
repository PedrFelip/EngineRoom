//! Native Rune kit desktop shell: toolbar, resizable sidebar and typed navigation.
use super::ui;
use crate::state::{Page, State};
use rune::{Cx, El, col};
use rune_kit::{
    Kit,
    shell::{Sidebar, SidebarRow, SidebarSection, SidebarToggle, SplitView, Toolbar},
};
use rune_theme_opencode::icons;
fn sidebar(s: &State, k: &Kit) -> El {
    let nav = |page: Page, key: &str, title: &str, icon: &'static rune::Icon| {
        SidebarRow::new(title)
            .icon(icon)
            .selected(s.page == page)
            .on_select(move |s: &mut State, _| s.navigate(page))
            .build(k)
            .key(key)
    };
    Sidebar::new()
        .header(
            ui::label("engineroom", 19.0, k.c.text_base)
                .px(12.0)
                .py(12.0),
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
        .section(SidebarSection::new("NESTA SESSÃO").row(nav(
            Page::Review,
            "review-nav",
            "Partida atual",
            &icons::BRANCH,
        )))
        .footer(
            SidebarRow::new("Configurações")
                .icon(&icons::SETTINGS_GEAR)
                .selected(s.page == Page::Settings)
                .on_select(|s: &mut State, _| s.open_settings())
                .build(k)
                .key("settings-nav"),
        )
        .footer(
            col()
                .gap(5.0)
                .px(12.0)
                .py(8.0)
                .child(ui::label("Prévia local em Rune", 10.0, k.c.text_muted))
                .child(ui::label("EngineRoom · laboratório", 10.0, k.c.text_faint)),
        )
        .build(k)
}
pub fn view(s: &State, cx: &mut Cx<State>, main: El, k: &Kit) -> El {
    SplitView::new(main)
        .sidebar(&s.sidebar, |s: &mut State| &mut s.sidebar, sidebar(s, k))
        .toolbar(Toolbar::new().title(s.page.title()).trailing(ui::label(
            "RUNE  /  EXPERIMENTAL",
            10.0,
            k.c.text_muted,
        )))
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(8.0)
        .toolbar_in_panel()
        .sidebar_toggle(SidebarToggle::new().in_header())
        .reduced_motion(s.preferences.reduced_motion || cx.is_reduced_motion())
        .build(k)
}
