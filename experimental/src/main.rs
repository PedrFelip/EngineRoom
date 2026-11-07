//! EngineRoom's experimental native frontend, composed with Rune and OpenCode's kit.
mod components;
mod game;
mod layout;
mod review_data;
mod state;
mod theme;
mod variation;

use rune::{AppSpec, Cx, El};
use state::{Appearance, Page, SettingsSection, State};
fn view(s: &State, cx: &mut Cx<State>) -> El {
    cx.reduced_motion(s.preferences.reduced_motion);
    let k = theme::kit(s.preferences.appearance);
    let mut layers = Vec::new();
    let layout = layout::Layout::new(cx.layout_size("page-body").unwrap_or([984.0, 836.0]));
    let content = match s.page {
        Page::Home => components::home::view(s, k, layout),
        Page::Configure => components::home::configure(s, k, layout),
        Page::Review => components::review::view(s, k, layout, &mut layers),
        Page::History => components::history::view(s, k, layout),
        Page::Settings => components::settings::view(s, k, layout, &mut layers),
    };
    rune_kit::root(
        components::shell::view(s, cx, content.key("page-body").fill(k.c.page), k),
        layers,
    )
}
fn spec() -> AppSpec<State> {
    AppSpec::<State> {
        min_size: Some((720.0, 560.0)),
        fonts: theme::FONTS,
        families: theme::FAMILIES,
        scenes: &[
            ("review", |s| s.page = Page::Review),
            ("opening", |s| {
                s.page = Page::Review;
                s.ply = s.game.moves.len();
            }),
            ("summary", |s| {
                s.page = Page::Review;
                s.ply = s.game.moves.len();
                s.review_tab = state::ReviewTab::Summary;
            }),
            ("demo-analysis", |s| {
                s.page = Page::Review;
                s.go_to(8);
                s.review_tab = state::ReviewTab::Analysis;
            }),
            ("demo-summary", |s| {
                s.page = Page::Review;
                s.go_to(11);
                s.review_tab = state::ReviewTab::Summary;
            }),
            ("demo-collapsed", |s| {
                s.page = Page::Review;
                s.go_to(11);
                s.sidebar.set_open(false);
            }),
            ("demo-progress", |s| {
                s.page = Page::Review;
                s.mock_status = state::MockStatus::Running;
            }),
            ("demo-error", |s| {
                s.page = Page::Review;
                s.mock_status = state::MockStatus::Failed;
            }),
            ("preview", |s| {
                s.page = Page::Review;
                s.review_mode = state::ReviewMode::Preview;
            }),
            ("history", |s| s.page = Page::History),
            ("configure", |s| {
                s.load_demo();
                s.prepare();
            }),
            ("manual", |s| {
                s.load_demo();
                s.prepare();
                s.profile = state::Profile::Manual;
            }),
            ("settings", |s| s.open_settings()),
            ("settings-light", |s| {
                s.open_settings();
                s.preferences.appearance = Appearance::Light;
            }),
            ("settings-analysis", |s| {
                s.open_settings();
                s.section = SettingsSection::Analysis;
            }),
            ("settings-storage", |s| {
                s.open_settings();
                s.section = SettingsSection::Storage;
            }),
        ],
        poll: Some(|s, now, tx| {
            let changed = s.tick(now);
            if s.playing || s.mock_status == state::MockStatus::Running {
                tx.wake_after(std::time::Duration::from_millis(50));
            }
            changed
        }),
        on_key: Some(|s, key, _| {
            if rune_kit::shell::is_sidebar_toggle(key) {
                s.sidebar.toggle();
                return true;
            }
            if s.page != Page::Review {
                return false;
            }
            match key.key {
                rune::Key::Right if key.mods.shift => s.next_critical(),
                rune::Key::Left if key.mods.shift => s.previous_critical(),
                rune::Key::Right => s.next(),
                rune::Key::Left => s.previous(),
                rune::Key::Home => s.first(),
                rune::Key::End => s.last(),
                _ => return false,
            }
            true
        }),
        ..AppSpec::new("EngineRoom · Rune", (1220.0, 900.0), State::default, view)
    }
}
fn main() {
    rune::run(spec());
}
#[cfg(test)]
mod tests;
