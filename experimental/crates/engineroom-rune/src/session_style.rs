//! Named folders for sidebar presentation. The workspace retains analysis ownership.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rune::{El, PointerPress, el, icon};
use rune_kit::Kit;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    state::{State, WorkspaceReview},
    theme,
};

/// A harmonised swatch from the app's theme scales. `None` is ordinary (no marker).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SessionColor {
    #[default]
    None,
    Blue,
    Green,
    Amber,
    Red,
    Violet,
}
impl SessionColor {
    /// Stable order used by the folder colour picker.
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::Blue,
        Self::Green,
        Self::Amber,
        Self::Red,
        Self::Violet,
    ];

    /// Persistent, locale-independent identifier.
    pub fn slug(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Blue => "blue",
            Self::Green => "green",
            Self::Amber => "amber",
            Self::Red => "red",
            Self::Violet => "violet",
        }
    }
    fn from_slug(slug: &str) -> Self {
        match slug.to_ascii_lowercase().as_str() {
            "blue" | "azul" => Self::Blue,
            "green" | "verde" => Self::Green,
            "amber" | "ambar" | "âmbar" => Self::Amber,
            "red" | "vermelho" => Self::Red,
            "violet" | "violeta" => Self::Violet,
            _ => Self::None,
        }
    }
    /// Human label for menus and group headers.
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "Sem cor",
            Self::Blue => "Azul",
            Self::Green => "Verde",
            Self::Amber => "Âmbar",
            Self::Red => "Vermelho",
            Self::Violet => "Violeta",
        }
    }
}
impl Serialize for SessionColor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.slug())
    }
}
impl<'de> Deserialize<'de> for SessionColor {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // An unknown colour must not invalidate the whole preferences document.
        let value = serde_json::Value::deserialize(deserializer)?;
        Ok(value.as_str().map_or(Self::None, Self::from_slug))
    }
}

/// A named folder has its own identity; multiple folders may share a colour.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionFolder {
    pub id: String,
    pub name: String,
    pub color: SessionColor,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(from = "StoredPresentation")]
pub struct SessionPresentation {
    pub folders: Vec<SessionFolder>,
    pub collapsed_folders: BTreeSet<String>,
}

/// Legacy colour labels become named folders exactly once on loading.
#[derive(Default, Deserialize)]
#[serde(default)]
struct StoredPresentation {
    folders: Vec<SessionFolder>,
    collapsed_folders: BTreeSet<String>,
    colors: BTreeMap<String, SessionColor>,
    collapsed_groups: BTreeSet<SessionColor>,
}
impl From<StoredPresentation> for SessionPresentation {
    fn from(stored: StoredPresentation) -> Self {
        let mut result = Self {
            folders: stored.folders,
            collapsed_folders: stored.collapsed_folders,
        };
        for color in stored.colors.into_values().collect::<BTreeSet<_>>() {
            if color == SessionColor::None {
                continue;
            }
            let id = format!("legacy-{}", color.slug());
            if !result.folders.iter().any(|folder| folder.id == id) {
                result.folders.push(SessionFolder {
                    id: id.clone(),
                    name: color.label().into(),
                    color,
                });
            }
            if stored.collapsed_groups.contains(&color) {
                result.collapsed_folders.insert(id.clone());
            }
        }
        result
            .collapsed_folders
            .retain(|id| result.folders.iter().any(|folder| &folder.id == id));
        result
    }
}

#[derive(Clone, Debug)]
pub struct FolderEditor {
    pub id: Option<String>,
    pub session: Option<u64>,
    pub name: rune::TextEdit,
    pub color: SessionColor,
}

/// Decorative colour swatch; folder names supply the accessible identity.
pub fn marker(color: SessionColor, k: &Kit) -> El {
    el().key(format!("session-swatch-{}", color.slug()))
        .size(14.0, 14.0)
        .center()
        .children((color != SessionColor::None).then(|| {
            el().size(7.0, 7.0)
                .radius(3.5)
                .fill(theme::session_swatch(color.slug(), k.dark))
                .border(0.5, k.c.border_base)
        }))
        .a11y_hidden()
        .inert()
}

/// Folder silhouette for section headers, including folders without an accent colour.
pub fn folder_marker(color: SessionColor, open: bool, k: &Kit) -> El {
    icon(
        if open {
            &rune_kit::glyphs::FOLDER_OPEN
        } else {
            &rune_kit::glyphs::FOLDER
        },
        16.0,
    )
    .color(if color == SessionColor::None {
        k.c.icon_muted
    } else {
        theme::session_swatch(color.slug(), k.dark)
    })
    .a11y_hidden()
    .inert()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DragItem {
    Session(u64),
    Folder(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertSide {
    Before,
    After,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionDropTarget {
    Folder(String),
    Unfiled,
    Reorder(DragItem, InsertSide),
}

#[derive(Clone, Debug)]
pub struct SessionDrag {
    pub item: DragItem,
    pub point: [f32; 2],
    pub target: Option<SessionDropTarget>,
}

fn relative_position<T: PartialEq>(
    items: impl Iterator<Item = T>,
    source: T,
    target: T,
) -> Option<InsertSide> {
    if source == target {
        return None;
    }
    let mut matches = items.filter(|item| *item == source || *item == target);
    let first = matches.next()?;
    matches.next()?;
    Some(if first == source {
        InsertSide::After
    } else {
        InsertSide::Before
    })
}

fn move_to<T>(items: &mut Vec<T>, source: impl Fn(&T) -> bool, target: impl Fn(&T) -> bool) {
    if let Some(from) = items.iter().position(source)
        && let Some(to) = items.iter().position(target)
        && from != to
    {
        let item = items.remove(from);
        items.insert(to, item);
    }
}

pub struct SessionGroup<'a> {
    pub folder: Option<&'a SessionFolder>,
    pub entries: Vec<&'a WorkspaceReview>,
}

fn display_groups<'a>(
    workspace: &'a [WorkspaceReview],
    presentation: &'a SessionPresentation,
    order: &BTreeMap<String, Vec<u64>>,
) -> Vec<SessionGroup<'a>> {
    debug_assert!(workspace.windows(2).all(|pair| pair[0].key < pair[1].key));
    let mut groups = Vec::with_capacity(presentation.folders.len() + 1);
    let mut folder_indices = HashMap::with_capacity(presentation.folders.len());
    let mut placed = HashSet::new();
    for folder in &presentation.folders {
        folder_indices.insert(folder.id.as_str(), groups.len());
        let entries = order
            .get(&folder.id)
            .into_iter()
            .flatten()
            .filter_map(|&key| workspace_entry(workspace, key))
            .filter(|entry| {
                entry.folder_id.as_deref() == Some(folder.id.as_str()) && placed.insert(entry.key)
            })
            .collect();
        groups.push(SessionGroup {
            folder: Some(folder),
            entries,
        });
    }

    let mut unfiled = SessionGroup {
        folder: None,
        entries: vec![],
    };
    for entry in workspace
        .iter()
        .filter(|entry| !placed.contains(&entry.key))
    {
        match entry
            .folder_id
            .as_deref()
            .and_then(|id| folder_indices.get(id))
        {
            Some(&index) => groups[index].entries.push(entry),
            None => unfiled.entries.push(entry),
        }
    }
    groups.push(unfiled);
    groups
}

fn workspace_entry(workspace: &[WorkspaceReview], key: u64) -> Option<&WorkspaceReview> {
    // The workspace appends monotonic keys and retains their order on close.
    let index = workspace
        .binary_search_by_key(&key, |entry| entry.key)
        .ok()?;
    Some(&workspace[index])
}

impl State {
    pub fn session_groups(&self) -> Vec<SessionGroup<'_>> {
        display_groups(
            &self.workspace,
            &self.preferences.sessions,
            &self.session_order,
        )
    }

    pub fn session_drop_target(&self, item: &DragItem, path: &str) -> Option<SessionDropTarget> {
        let folder = path
            .split('/')
            .find_map(|key| key.strip_prefix("session-folder-"));
        match item {
            DragItem::Folder(source) => {
                let target = folder?;
                let side = relative_position(
                    self.preferences
                        .sessions
                        .folders
                        .iter()
                        .map(|folder| folder.id.as_str()),
                    source.as_str(),
                    target,
                )?;
                Some(SessionDropTarget::Reorder(
                    DragItem::Folder(target.into()),
                    side,
                ))
            }
            DragItem::Session(source) => {
                let current = self
                    .session_folder(*source)
                    .map(|folder| folder.id.as_str());
                if let Some(folder) = folder {
                    if current != Some(folder) {
                        return Some(SessionDropTarget::Folder(folder.into()));
                    }
                    let target = path
                        .split('/')
                        .find_map(|key| key.strip_prefix("session-")?.parse::<u64>().ok())?;
                    if target == *source
                        || workspace_entry(&self.workspace, target)?
                            .folder_id
                            .as_deref()
                            != Some(folder)
                    {
                        return None;
                    }
                    let order = self.session_order.get(folder);
                    let visual_key = |key| {
                        let rank =
                            order.and_then(|keys| keys.iter().position(|&entry| entry == key));
                        (rank.unwrap_or(usize::MAX), key)
                    };
                    let side = if visual_key(*source) < visual_key(target) {
                        InsertSide::After
                    } else {
                        InsertSide::Before
                    };
                    Some(SessionDropTarget::Reorder(DragItem::Session(target), side))
                } else if current.is_some() && path.split('/').any(|key| key == "session-unfiled") {
                    Some(SessionDropTarget::Unfiled)
                } else {
                    None
                }
            }
        }
    }

    pub fn apply_session_drop(&mut self, item: DragItem, target: SessionDropTarget) {
        match (item, target) {
            (DragItem::Session(key), SessionDropTarget::Folder(id)) => {
                self.move_session(key, Some(id))
            }
            (DragItem::Session(key), SessionDropTarget::Unfiled) => self.move_session(key, None),
            (DragItem::Folder(source), SessionDropTarget::Reorder(DragItem::Folder(target), _)) => {
                move_to(
                    &mut self.preferences.sessions.folders,
                    |folder| folder.id == source,
                    |folder| folder.id == target,
                );
            }
            (
                DragItem::Session(source),
                SessionDropTarget::Reorder(DragItem::Session(target), _),
            ) => {
                let Some(folder) = self.session_folder(source) else {
                    return;
                };
                if self.session_folder(target) != Some(folder) {
                    return;
                }
                let id = folder.id.clone();
                let order = self.session_order.entry(id.clone()).or_insert_with(|| {
                    self.workspace
                        .iter()
                        .filter(|entry| entry.folder_id.as_deref() == Some(&id))
                        .map(|entry| entry.key)
                        .collect()
                });
                move_to(order, |key| *key == source, |key| *key == target);
            }
            _ => {}
        }
    }

    pub fn forget_session_order(&mut self, key: u64) {
        if let Some(id) = self.session_folder(key).map(|folder| folder.id.clone())
            && let Some(order) = self.session_order.get_mut(&id)
        {
            order.retain(|entry| *entry != key);
        }
    }

    pub fn session_folder(&self, key: u64) -> Option<&SessionFolder> {
        let entry = workspace_entry(&self.workspace, key)?;
        let id = entry.folder_id.as_ref()?;
        self.preferences
            .sessions
            .folders
            .iter()
            .find(|folder| &folder.id == id)
    }
    pub fn move_session(&mut self, key: u64, folder: Option<String>) {
        self.session_menu = None;
        if folder.as_ref().is_some_and(|id| {
            !self
                .preferences
                .sessions
                .folders
                .iter()
                .any(|folder| &folder.id == id)
        }) {
            return;
        }
        let Some(index) = self.workspace.iter().position(|entry| entry.key == key) else {
            return;
        };
        if self.workspace[index].folder_id != folder {
            self.forget_session_order(key);
            if let Some(order) = folder
                .as_ref()
                .and_then(|id| self.session_order.get_mut(id))
            {
                order.push(key);
            }
        }
        if let Some(id) = &folder {
            self.preferences.sessions.collapsed_folders.remove(id);
        }
        self.workspace[index].folder_id = folder;
    }
    pub fn edit_folder(&mut self, id: Option<String>, session: Option<u64>) {
        self.session_menu = None;
        self.folder_menu = None;
        let folder = id.as_ref().and_then(|id| {
            self.preferences
                .sessions
                .folders
                .iter()
                .find(|folder| &folder.id == id)
        });
        self.folder_editor = Some(FolderEditor {
            id: id.clone(),
            session,
            name: rune::TextEdit::new(folder.map_or("", |folder| folder.name.as_str())),
            color: folder.map_or(SessionColor::Blue, |folder| folder.color),
        });
    }
    pub fn save_folder(&mut self) {
        let Some(editor) = &self.folder_editor else {
            return;
        };
        let name = editor.name.text.trim().to_string();
        if name.is_empty() || name.chars().count() > 80 {
            return;
        }
        let editor = self.folder_editor.take().expect("editor exists");
        let id = if let Some(id) = editor.id {
            let Some(folder) = self
                .preferences
                .sessions
                .folders
                .iter_mut()
                .find(|folder| folder.id == id)
            else {
                return;
            };
            folder.name = name;
            folder.color = editor.color;
            id
        } else {
            let mut number = 1_u64;
            let id = loop {
                let id = format!("folder-{number}");
                if !self
                    .preferences
                    .sessions
                    .folders
                    .iter()
                    .any(|folder| folder.id == id)
                {
                    break id;
                }
                number += 1;
            };
            self.preferences.sessions.folders.push(SessionFolder {
                id: id.clone(),
                name,
                color: editor.color,
            });
            id
        };
        if let Some(key) = editor.session {
            self.move_session(key, Some(id));
        }
    }
    pub fn delete_folder(&mut self, id: &str) {
        self.folder_menu = None;
        self.session_order.remove(id);
        self.preferences
            .sessions
            .folders
            .retain(|folder| folder.id != id);
        for entry in &mut self.workspace {
            if entry.folder_id.as_deref() == Some(id) {
                entry.folder_id = None;
            }
        }
        self.preferences.sessions.collapsed_folders.remove(id);
        self.folder_editor = None;
    }
    pub fn toggle_folder(&mut self, id: String) {
        if !self.preferences.sessions.collapsed_folders.remove(&id) {
            self.preferences.sessions.collapsed_folders.insert(id);
        }
    }
    pub fn open_session_menu(&mut self, key: u64, press: PointerPress) {
        self.folder_menu = None;
        self.session_menu = Some((key, press));
    }
    pub fn open_folder_menu(&mut self, id: String, press: PointerPress) {
        self.session_menu = None;
        self.folder_menu = Some((id, press));
    }
    pub fn close_session_menu(&mut self) {
        self.session_menu = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create(state: &mut State, name: &str, color: SessionColor) -> String {
        state.edit_folder(None, None);
        let editor = state.folder_editor.as_mut().unwrap();
        editor.name.replace_text(name);
        editor.color = color;
        state.save_folder();
        state
            .preferences
            .sessions
            .folders
            .last()
            .unwrap()
            .id
            .clone()
    }

    #[test]
    fn legacy_colors_migrate_and_new_documents_roundtrip() {
        let old = r#"{"colors":{"a":"blue","b":"blue","c":"rosa","d":"green"},"collapsed_groups":["blue","unknown"],"group_by_color":true}"#;
        let migrated: SessionPresentation = serde_json::from_str(old).unwrap();
        assert_eq!(migrated.folders.len(), 2);
        assert!(migrated.folders.iter().any(|folder| folder.name == "Azul"));
        assert!(migrated.collapsed_folders.contains("legacy-blue"));
        let json = serde_json::to_string(&migrated).unwrap();
        assert!(!json.contains("collapsed_groups"));
        assert_eq!(
            serde_json::from_str::<SessionPresentation>(&json).unwrap(),
            migrated
        );
        let empty: SessionPresentation = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, SessionPresentation::default());
    }

    #[test]
    fn folders_with_the_same_color_are_independent_and_keep_empty_folders() {
        let mut state = State::fixture();
        let first = create(&mut state, "Aberturas", SessionColor::Blue);
        let second = create(&mut state, "Torneio", SessionColor::Blue);
        assert_ne!(first, second);
        let key = state.workspace[0].key;
        let original: Vec<_> = state.workspace.iter().map(|entry| entry.key).collect();
        state.move_session(key, Some(first.clone()));
        state.toggle_folder(second.clone());
        state.move_session(key, Some(second.clone()));
        assert_eq!(state.session_folder(key).unwrap().id, second);
        assert!(
            !state
                .preferences
                .sessions
                .collapsed_folders
                .contains(&second)
        );
        let groups = state.session_groups();
        assert!(
            groups
                .iter()
                .find(|group| group.folder.is_some_and(|folder| folder.id == first))
                .unwrap()
                .entries
                .is_empty()
        );
        assert_eq!(
            state
                .workspace
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>(),
            original
        );
        state.move_session(key, None);
        assert!(state.session_folder(key).is_none());
    }

    #[test]
    fn explicit_order_tracks_moves_and_closes() {
        let mut state = crate::fixtures::workspace_scene();
        let folder = create(&mut state, "Estudos", SessionColor::Blue);
        let first = state.workspace[0].key;
        let second = state.workspace[1].key;
        state.move_session(first, Some(folder.clone()));
        state.move_session(second, Some(folder.clone()));
        state.apply_session_drop(
            DragItem::Session(first),
            SessionDropTarget::Reorder(DragItem::Session(second), InsertSide::After),
        );
        let keys = |state: &State| {
            state.session_groups()[0]
                .entries
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&state), [second, first]);

        state.move_session(second, None);
        assert_eq!(keys(&state), [first]);
        state.move_session(second, Some(folder));
        assert_eq!(keys(&state), [first, second]);
        state.close_review(first);
        assert_eq!(keys(&state), [second]);
        assert_eq!(state.workspace[0].key, second);
    }

    #[test]
    fn grouping_keeps_sessions_visible_when_order_is_incomplete() {
        let mut state = crate::fixtures::workspace_scene();
        let folder = create(&mut state, "Estudos", SessionColor::Blue);
        let first = state.workspace[0].key;
        let second = state.workspace[1].key;
        state.move_session(first, Some(folder.clone()));
        state.move_session(second, Some(folder.clone()));
        state
            .session_order
            .insert(folder, vec![second, second, u64::MAX]);

        let groups = state.session_groups();
        assert_eq!(
            groups[0]
                .entries
                .iter()
                .map(|entry| entry.key)
                .collect::<Vec<_>>(),
            [second, first]
        );
        assert!(groups[1].entries.is_empty());
    }

    #[test]
    fn editing_and_deleting_folder_preserve_sessions() {
        let mut state = State::fixture();
        let id = create(&mut state, "  Aberturas  ", SessionColor::Blue);
        let key = state.workspace[0].key;
        state.move_session(key, Some(id.clone()));
        state.edit_folder(Some(id.clone()), None);
        let editor = state.folder_editor.as_mut().unwrap();
        editor.name.replace_text("Estudos");
        editor.color = SessionColor::Red;
        state.save_folder();
        let folder = state.session_folder(key).unwrap();
        assert_eq!(folder.name, "Estudos");
        assert_eq!(folder.color, SessionColor::Red);
        state.toggle_folder(id.clone());
        let count = state.workspace.len();
        state.delete_folder(&id);
        assert!(state.session_folder(key).is_none());
        assert_eq!(state.workspace.len(), count);
        assert!(state.preferences.sessions.collapsed_folders.is_empty());
        assert!(
            state
                .workspace
                .iter()
                .all(|entry| entry.folder_id.is_none())
        );
    }

    #[test]
    fn invalid_names_and_unknown_destinations_do_not_change_preferences() {
        let mut state = State::fixture();
        for name in ["   ".to_string(), "a".repeat(81)] {
            state.edit_folder(None, None);
            state
                .folder_editor
                .as_mut()
                .unwrap()
                .name
                .replace_text(&name);
            state.save_folder();
            assert!(state.folder_editor.is_some());
            assert!(state.preferences.sessions.folders.is_empty());
        }
        state.move_session(state.workspace[0].key, Some("missing".into()));
        assert!(
            state
                .workspace
                .iter()
                .all(|entry| entry.folder_id.is_none())
        );
    }

    #[test]
    fn newly_opened_sessions_are_unfiled_even_for_the_same_pgn() {
        let mut state = State::fixture();
        let id = create(&mut state, "Estudos", SessionColor::Violet);
        let first = state.workspace[0].key;
        let config = state.workspace[0].config.clone();
        state.move_session(first, Some(id.clone()));
        state.register_review(config.clone());
        let second = state.active_workspace.unwrap();
        assert!(state.session_folder(second).is_none());
        assert_eq!(state.session_folder(first).unwrap().id, id);
        state.navigate(crate::state::Page::Home);
        state.activate_review(first);
        state.navigate(crate::state::Page::Home);
        assert_eq!(state.session_folder(first).unwrap().id, id);
        state.close_review(first);
        state.register_review(config);
        assert!(
            state
                .session_folder(state.active_workspace.unwrap())
                .is_none()
        );
    }
    #[test]
    fn folder_colors_remain_visible_in_both_themes() {
        let contrast = |a: rune::Color, b: rune::Color| {
            let (a, b) = (a.luminance(), b.luminance());
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for dark in [true, false] {
            let kit = crate::theme::kit(if dark {
                crate::state::Appearance::Dark
            } else {
                crate::state::Appearance::Light
            });
            for color in SessionColor::ALL
                .into_iter()
                .filter(|color| *color != SessionColor::None)
            {
                for surface in [kit.c.layer_01, kit.c.layer_02] {
                    assert!(contrast(theme::session_swatch(color.slug(), dark), surface) >= 2.0);
                }
            }
        }
    }
}
