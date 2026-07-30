// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use ::engine_core::ui::parser::parse_node_template;
use mod_api::*;
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};

const MOD_ID: &str = "mod_menu";
const NO_SELECTION: &str = "-";
const RUNTIME_LAYOUT_ASSET: &str = "asset/mod_menu/ui/layout/better_mod_menu_runtime";
const RUNTIME_SLOT_ASSET: &str = "asset/mod_menu/ui/layout/mods_component/mod_slot_runtime";
const OWNED_ROW_ASSET: &str = "asset/mod_menu/ui/layout/mods_component/owned_mod_row_runtime";
const SETTING_ROW_ASSET: &str = "asset/mod_menu/ui/layout/mods_component/mod_setting_row_runtime";
const SETTING_FILE_CARDS_ASSET: &str =
    "asset/mod_menu/ui/layout/mods_component/mod_file_cards_row_runtime";
const SETTING_CATEGORY_ASSET: &str =
    "asset/mod_menu/ui/layout/mods_component/mod_setting_category_runtime";
const SETTINGS_MANIFEST_FILE: &str = "better_mod_menu.json";
const GAME_VERSION: &str = "0.5.3";
const LABEL_RUNNER_TYPE: &str = "engine_ui::runner::label::LabelRunner";
const COLOR_RUNNER_TYPE: &str = "engine_ui::runner::color::ColorRunner";
const COLOR_SELECTABLE_RUNNER_TYPE: &str = "engine_ui::runner::selectable::ColorSelectableRunner";
const TEXT_EDIT_RUNNER_TYPE: &str = "engine_ui::runner::text_edit::TextEditRunner";

const FILTER_ALL: usize = 0;
const FILTER_CODE: usize = 1;
const FILTER_WORKSHOP: usize = 2;

const STATUS_NONE: usize = 0;
const STATUS_RESTART: usize = 1;
const STATUS_WARNING: usize = 2;
const STATUS_UPDATE: usize = 3;
const TOGGLE_TRANSITION_FRAMES: f32 = 12.0;
const SETTINGS_STATUS_FRAMES: usize = 90;

const KEY_UP: i32 = 0x26;
const KEY_DOWN: i32 = 0x28;
const KEY_HOME: i32 = 0x24;
const KEY_END: i32 = 0x23;
const KEY_E: i32 = 0x45;
const KEY_D: i32 = 0x44;

const KEY_MASK_UP: usize = 1 << 0;
const KEY_MASK_DOWN: usize = 1 << 1;
const KEY_MASK_HOME: usize = 1 << 2;
const KEY_MASK_END: usize = 1 << 3;
const KEY_MASK_ENABLE: usize = 1 << 4;
const KEY_MASK_DISABLE: usize = 1 << 5;

#[derive(Clone, Copy)]
struct ToggleVisual {
    from_enabled: bool,
    to_enabled: bool,
    progress: f32,
}

struct ToggleTransition {
    name: String,
    last_enabled: bool,
    from_enabled: bool,
    progress: f32,
}

#[derive(Clone, Deserialize)]
struct ModSettingsManifest {
    schema_version: u32,
    mod_id: String,
    name: String,
    #[serde(default = "default_storage")]
    storage: String,
    #[serde(default = "default_settings_file")]
    settings_file: String,
    #[serde(default = "default_actions_file")]
    actions_file: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    display: ModManifestDisplay,
    #[serde(default)]
    controls: Vec<ModSettingControl>,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ModSettingControl {
    Toggle {
        key: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        default: bool,
    },
    Choice {
        key: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        options: Vec<ModSettingOption>,
    },
    Button {
        action: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        #[serde(default = "default_button_label")]
        button_label: String,
    },
    FileCards {
        action: String,
        label: String,
        #[serde(default)]
        category: String,
        #[serde(default)]
        description: String,
        directory: String,
        #[serde(default)]
        filename_contains: String,
        #[serde(default)]
        extension: String,
        #[serde(default = "default_file_card_limit")]
        limit: usize,
    },
}

#[derive(Clone, Deserialize)]
struct ModSettingOption {
    label: String,
    value: JsonValue,
}

#[derive(Clone, Default, Deserialize)]
struct ModManifestDisplay {
    title: Option<String>,
    author: Option<String>,
    version: Option<String>,
    source: Option<String>,
    dependencies: Option<Vec<String>>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct ModInfoIdentity {
    #[serde(default)]
    mod_id: String,
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    dependencies: Vec<ModInfoDependency>,
}

#[derive(Clone, Deserialize)]
struct ModInfoDependency {
    mod_id: String,
    #[serde(default)]
    version: String,
}

#[derive(Clone)]
struct SettingsManifestEntry {
    directory: PathBuf,
    manifest: ModSettingsManifest,
}

struct ModVisualEntry {
    directory: PathBuf,
    mod_id: String,
    name: String,
    version: String,
    dependencies: Vec<ModInfoDependency>,
}

#[derive(Default)]
struct ModSettingsCatalog {
    entries: Vec<SettingsManifestEntry>,
    visuals: Vec<ModVisualEntry>,
    enabled_mod_ids: Vec<String>,
    dependency_warnings: Vec<(String, String)>,
    active_name: String,
    active_entry: Option<usize>,
    active_thumbnail: Option<String>,
    active_banner: Option<String>,
    values: JsonMap<String, JsonValue>,
    status: String,
    status_row: Option<usize>,
    status_frames: usize,
    poll_tick: usize,
    show_settings: bool,
    dependency_tooltip: String,
}

#[derive(Clone, Copy)]
enum SettingAction {
    Toggle,
    Previous,
    Next,
    Run,
    RunIndex(usize),
    ShowOverview,
    ShowSettings,
}

fn default_storage() -> String {
    "mod".to_owned()
}

fn default_settings_file() -> String {
    "settings.json".to_owned()
}

fn default_actions_file() -> String {
    "better_mod_menu.actions.json".to_owned()
}

fn default_button_label() -> String {
    "Run".to_owned()
}

fn default_file_card_limit() -> usize {
    5
}

struct BetterModMenuExtension {
    popup_open: AtomicBool,
    pending_selection: AtomicUsize,
    selected_index: AtomicUsize,
    key_state: AtomicUsize,
    text_key_state: AtomicUsize,
    shortcut_down: AtomicBool,
    restarting: AtomicBool,
    mouse_down: AtomicBool,
    toggle_reselect_pending: AtomicBool,
    poll_tick: AtomicUsize,
    styled_row_count: AtomicUsize,
    enabled_count: AtomicUsize,
    search_focused: AtomicBool,
    filter_hovered: AtomicBool,
    filter: AtomicUsize,
    query: Mutex<String>,
    selected_name: Mutex<String>,
    baseline_mod_states: Mutex<Vec<(String, bool)>>,
    toggle_transitions: Mutex<Vec<ToggleTransition>>,
    settings_catalog: Mutex<ModSettingsCatalog>,
}

impl ModExtension for BetterModMenuExtension {
    fn post_update(&self, _scene: &mut Scene, ui: &mut GameUI, assets: &mut Assets, _dt: f32) {
        let open = node_is_visible(&ui.root, "mods_popup");
        let was_open = self.popup_open.swap(open, Ordering::AcqRel);
        if !open {
            self.key_state.store(0, Ordering::Release);
            self.text_key_state.store(0, Ordering::Release);
            self.shortcut_down.store(false, Ordering::Release);
            self.pending_selection.store(0, Ordering::Release);
            self.toggle_reselect_pending.store(false, Ordering::Release);
            self.styled_row_count.store(0, Ordering::Release);
            self.search_focused.store(false, Ordering::Release);
            self.filter_hovered.store(false, Ordering::Release);
            if let Ok(mut transitions) = self.toggle_transitions.lock() {
                transitions.clear();
            }
            return;
        }

        if !ensure_runtime_ui(&mut ui.root, assets) {
            return;
        }
        let installed = mod_row_count(&ui.root);
        if installed == 0 {
            return;
        }
        let styled_row_count = self.styled_row_count.swap(installed, Ordering::AcqRel);
        if !was_open || styled_row_count != installed {
            apply_mod_row_layouts(&mut ui.root, assets);
        }
        self.capture_baseline_mod_states(&ui.root, assets);
        if !was_open {
            self.pending_selection.store(2, Ordering::Release);
            if let Ok(mut catalog) = self.settings_catalog.lock() {
                *catalog = discover_settings_catalog();
            }
        }
        let poll = self.poll_tick.fetch_add(1, Ordering::Relaxed);
        if poll.is_multiple_of(30) || !was_open {
            self.enabled_count
                .store(enabled_mod_count().unwrap_or(0), Ordering::Release);
            if let Ok(mut catalog) = self.settings_catalog.lock() {
                refresh_dependency_cache(&mut catalog);
            }
        }

        if self.pending_selection.load(Ordering::Acquire) == 0
            && !self.toggle_reselect_pending.load(Ordering::Acquire)
        {
            self.remember_current_selection(&ui.root, assets);
        }
        self.handle_pointer_input(ui, assets);
        self.handle_search_input(ui);

        let cursor = cursor_ui_position(ui);
        let hovered_filter = cursor.and_then(|(x, y)| filter_at_point(&ui.root, x, y));
        let hovered_setting = cursor
            .and_then(|(x, y)| setting_control_at_point(&ui.root, x, y))
            .is_some();
        let hovered_clickable = hovered_filter.is_some() || hovered_setting;
        self.filter_hovered
            .store(hovered_clickable, Ordering::Release);

        let query = self
            .query
            .lock()
            .map(|value| value.clone())
            .unwrap_or_default();
        let filter = self.filter.load(Ordering::Acquire);
        let visible = apply_mod_filter(&mut ui.root, assets, &query, filter);
        let selection_hint = self
            .selected_name
            .lock()
            .map(|name| name.clone())
            .unwrap_or_default();
        let baseline_mod_states = self
            .baseline_mod_states
            .lock()
            .map(|states| states.clone())
            .unwrap_or_default();
        let dependency_warnings = self
            .settings_catalog
            .lock()
            .map(|catalog| catalog.dependency_warnings.clone())
            .unwrap_or_default();
        let pending_restart = current_mod_states(&ui.root, assets)
            .iter()
            .any(|(name, enabled)| {
                baseline_mod_states
                    .iter()
                    .find(|(baseline_name, _)| baseline_name == name)
                    .is_some_and(|(_, baseline_enabled)| baseline_enabled != enabled)
            });
        let prefer_selection_hint = self.toggle_reselect_pending.load(Ordering::Acquire)
            || self.pending_selection.load(Ordering::Acquire) > 0;
        if let Ok(mut transitions) = self.toggle_transitions.lock() {
            sync_owned_rows(
                &mut ui.root,
                assets,
                &selection_hint,
                prefer_selection_hint,
                &baseline_mod_states,
                &dependency_warnings,
                &mut transitions,
            );
        }

        sync_menu_chrome(
            &mut ui.root,
            MenuChromeState {
                installed,
                visible,
                enabled: self.enabled_count.load(Ordering::Acquire),
                query: &query,
                filter,
                hovered_filter,
                pending_restart,
            },
        );
        if selected_mod_name(&ui.root, assets).is_some_and(|name| name != NO_SELECTION) {
            sync_selected_preview(&mut ui.root, assets);
        }
        let dependency_tooltip = if let Ok(mut catalog) = self.settings_catalog.lock() {
            sync_mod_settings(&mut ui.root, assets, &mut catalog);
            catalog.dependency_tooltip.clone()
        } else {
            String::new()
        };
        let tooltip = cursor.and_then(|(x, y)| {
            tooltip_at_point(
                &ui.root,
                assets,
                x,
                y,
                &dependency_tooltip,
                &dependency_warnings,
                pending_restart,
            )
        });
        sync_runtime_tooltip(&mut ui.root, tooltip.as_deref());

        let visible_indices = mod_visible_indices(&ui.root);
        if visible_indices.is_empty() {
            return;
        }
        let current = self.selected_index.load(Ordering::Acquire);
        if !visible_indices.contains(&current)
            && self.pending_selection.load(Ordering::Acquire) == 0
        {
            self.pending_selection.store(1, Ordering::Release);
        }

        let pending = self.pending_selection.load(Ordering::Acquire);
        if pending > 0 {
            self.pending_selection
                .store(pending.saturating_sub(1), Ordering::Release);
            if pending == 1 {
                let preferred = self
                    .selected_name
                    .lock()
                    .ok()
                    .and_then(|name| (!name.is_empty()).then(|| name.clone()))
                    .and_then(|name| mod_row_index_by_name(&ui.root, assets, &name))
                    .filter(|index| visible_indices.contains(index))
                    .unwrap_or(visible_indices[0]);
                self.select_row(ui, preferred, installed);
            }
            return;
        }

        self.handle_navigation(ui, installed, &visible_indices);
    }
}

impl BetterModMenuExtension {
    fn capture_baseline_mod_states(&self, root: &Node, assets: &Assets) {
        let Ok(mut baseline) = self.baseline_mod_states.lock() else {
            return;
        };
        if baseline.is_empty() {
            *baseline = current_mod_states(root, assets);
        }
    }

    fn remember_current_selection(&self, root: &Node, assets: &Assets) {
        let Some(name) = selected_mod_name(root, assets).filter(|name| name != NO_SELECTION) else {
            return;
        };
        let Some(index) = mod_row_index_by_name(root, assets, &name) else {
            return;
        };
        self.selected_index.store(index, Ordering::Release);
        if let Ok(mut selected_name) = self.selected_name.lock() {
            selected_name.clear();
            selected_name.push_str(&name);
        }
    }

    fn handle_pointer_input(&self, ui: &mut GameUI, assets: &Assets) {
        let down = left_mouse_down();
        let was_down = self.mouse_down.swap(down, Ordering::AcqRel);
        if !down {
            if was_down && self.toggle_reselect_pending.swap(false, Ordering::AcqRel) {
                self.pending_selection.store(3, Ordering::Release);
            }
            return;
        }
        if was_down {
            return;
        }
        let Some((x, y)) = cursor_ui_position(ui) else {
            return;
        };

        if node_contains_point(&ui.root, "owned_restart_button", x, y) {
            if !self.restarting.swap(true, Ordering::AcqRel) && !request_clean_restart() {
                self.restarting.store(false, Ordering::Release);
            }
            return;
        }

        if node_contains_point(&ui.root, "mod_menu_search_clear", x, y) {
            let changed = set_search_text(&mut ui.root, "");
            let _ = set_search_editing(&mut ui.root, true);
            self.search_focused.store(true, Ordering::Release);
            if changed {
                if let Ok(mut query) = self.query.lock() {
                    query.clear();
                }
                self.pending_selection.store(1, Ordering::Release);
            }
            return;
        }

        if node_contains_point(&ui.root, "mod_menu_search", x, y) {
            let _ = set_search_editing(&mut ui.root, true);
            self.search_focused.store(true, Ordering::Release);
            return;
        }

        if self.handle_setting_control_click(&ui.root, x, y) {
            let _ = set_search_editing(&mut ui.root, false);
            self.search_focused.store(false, Ordering::Release);
            self.text_key_state.store(0, Ordering::Release);
            return;
        }

        if let Some((index, name)) = mod_row_toggle_at_point(&ui.root, assets, x, y) {
            self.selected_index.store(index, Ordering::Release);
            if let Ok(mut selected_name) = self.selected_name.lock() {
                selected_name.clear();
                selected_name.push_str(&name);
            }
            self.toggle_reselect_pending.store(true, Ordering::Release);
            let _ = set_search_editing(&mut ui.root, false);
            self.search_focused.store(false, Ordering::Release);
            self.text_key_state.store(0, Ordering::Release);
            return;
        }

        if let Some(filter) = filter_at_point(&ui.root, x, y) {
            self.filter.store(filter, Ordering::Release);
            let _ = set_search_editing(&mut ui.root, false);
            self.search_focused.store(false, Ordering::Release);
            self.text_key_state.store(0, Ordering::Release);
            self.pending_selection.store(1, Ordering::Release);
            return;
        }
        let _ = set_search_editing(&mut ui.root, false);
        self.search_focused.store(false, Ordering::Release);
        self.text_key_state.store(0, Ordering::Release);
    }

    fn handle_setting_control_click(&self, root: &Node, x: f32, y: f32) -> bool {
        let Some((row_index, action)) = setting_control_at_point(root, x, y) else {
            return false;
        };
        let Ok(mut catalog) = self.settings_catalog.lock() else {
            return false;
        };
        apply_setting_action(&mut catalog, row_index, action)
    }

    fn handle_search_input(&self, ui: &mut GameUI) {
        let shortcut = ctrl_f_down();
        let previous = self.shortcut_down.swap(shortcut, Ordering::AcqRel);
        if shortcut && !previous {
            if let Some((x, y)) = find_node(&ui.root, "mod_menu_search").and_then(node_center) {
                let _ = post_ui_click(ui, x, y);
            }
            let _ = set_search_editing(&mut ui.root, true);
            self.search_focused.store(true, Ordering::Release);
        }

        let Some((text, editing)) = search_state(&ui.root) else {
            return;
        };
        if editing {
            self.search_focused.store(true, Ordering::Release);
        }
        let Ok(mut query) = self.query.lock() else {
            return;
        };
        let native_changed = *query != text;
        if native_changed {
            query.clear();
            query.push_str(&text);
            self.pending_selection.store(1, Ordering::Release);
        }

        let focused = self.search_focused.load(Ordering::Acquire);
        let keys = if focused {
            current_search_key_mask()
        } else {
            0
        };
        let previous = self.text_key_state.swap(keys, Ordering::AcqRel);
        let pressed = keys & !previous;
        if native_changed || pressed == 0 {
            return;
        }

        let mut changed = false;
        if pressed & 1 != 0 {
            changed = query.pop().is_some();
        } else if pressed & 2 != 0 && query.len() < 64 {
            query.push(' ');
            changed = true;
        } else {
            for index in 0..26 {
                if pressed & (1usize << (index + 2)) != 0 && query.len() < 64 {
                    query.push((b'a' + index as u8) as char);
                    changed = true;
                }
            }
            for index in 0..10 {
                if pressed & (1usize << (index + 28)) != 0 && query.len() < 64 {
                    query.push((b'0' + index as u8) as char);
                    changed = true;
                }
            }
        }
        if changed {
            let value = query.clone();
            let _ = set_search_text(&mut ui.root, &value);
            self.pending_selection.store(1, Ordering::Release);
        }
    }

    fn handle_navigation(&self, ui: &GameUI, installed: usize, visible_indices: &[usize]) {
        let keys = current_key_mask();
        let previous = self.key_state.swap(keys, Ordering::AcqRel);
        let pressed = keys & !previous;
        if pressed == 0 || self.search_focused.load(Ordering::Acquire) {
            return;
        }

        let current = self
            .selected_index
            .load(Ordering::Acquire)
            .min(installed - 1);
        let position = visible_indices
            .iter()
            .position(|index| *index == current)
            .unwrap_or(0);
        let target = if pressed & KEY_MASK_HOME != 0 {
            Some(visible_indices[0])
        } else if pressed & KEY_MASK_END != 0 {
            visible_indices.last().copied()
        } else if pressed & KEY_MASK_UP != 0 {
            Some(visible_indices[position.saturating_sub(1)])
        } else if pressed & KEY_MASK_DOWN != 0 {
            Some(visible_indices[(position + 1).min(visible_indices.len() - 1)])
        } else {
            None
        };

        if let Some(index) = target {
            self.select_row(ui, index, installed);
        } else if pressed & KEY_MASK_ENABLE != 0 {
            if self.click_row_control(ui, current, "enabled") {
                self.pending_selection.store(4, Ordering::Release);
            }
        } else if pressed & KEY_MASK_DISABLE != 0 && self.click_row_control(ui, current, "disabled")
        {
            self.pending_selection.store(4, Ordering::Release);
        }
    }

    fn select_row(&self, ui: &GameUI, index: usize, installed: usize) {
        let index = index.min(installed - 1);
        if let Some((x, y)) = mod_row_click_point(&ui.root, index) {
            if post_ui_click(ui, x, y) {
                self.selected_index.store(index, Ordering::Release);
            }
        }
    }

    fn click_row_control(&self, ui: &GameUI, index: usize, control_id: &str) -> bool {
        if let Some((x, y)) = mod_row_control_click_point(&ui.root, index, control_id) {
            return post_ui_click(ui, x, y);
        }
        false
    }
}

fn find_node<'a>(node: &'a Node, id: &str) -> Option<&'a Node> {
    if node.id == id {
        return Some(node);
    }
    node.child.iter().find_map(|child| find_node(child, id))
}

fn find_node_mut<'a>(node: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if node.id == id {
        return Some(node);
    }
    node.child
        .iter_mut()
        .find_map(|child| find_node_mut(child, id))
}

fn find_visible_node<'a>(node: &'a Node, id: &str, ancestors_visible: bool) -> Option<&'a Node> {
    let visible = ancestors_visible && node.visible;
    if node.id == id {
        return visible.then_some(node);
    }
    if !visible {
        return None;
    }
    node.child
        .iter()
        .find_map(|child| find_visible_node(child, id, visible))
}

fn node_is_visible(root: &Node, id: &str) -> bool {
    find_visible_node(root, id, true).is_some()
}

fn node_contains_point(root: &Node, id: &str, x: f32, y: f32) -> bool {
    let Some(node) = find_visible_node(root, id, true) else {
        return false;
    };
    node.rect.w > 0.0
        && node.rect.h > 0.0
        && x >= node.rect.x
        && x <= node.rect.x + node.rect.w
        && y >= node.rect.y
        && y <= node.rect.y + node.rect.h
}

fn filter_at_point(root: &Node, x: f32, y: f32) -> Option<usize> {
    [
        (
            FILTER_ALL,
            "mod_menu_filter_all",
            "mod_menu_filter_all_active",
        ),
        (
            FILTER_CODE,
            "mod_menu_filter_code",
            "mod_menu_filter_code_active",
        ),
        (
            FILTER_WORKSHOP,
            "mod_menu_filter_workshop",
            "mod_menu_filter_workshop_active",
        ),
    ]
    .into_iter()
    .find_map(|(filter, normal_id, active_id)| {
        (node_contains_point(root, normal_id, x, y) || node_contains_point(root, active_id, x, y))
            .then_some(filter)
    })
}

fn set_node_visible(node: &mut Node, id: &str, visible: bool) -> bool {
    if node.id == id {
        if node.visible != visible {
            node.visible = visible;
            node.runner.set_dirty(true);
        }
        return true;
    }
    node.child
        .iter_mut()
        .any(|child| set_node_visible(child, id, visible))
}

fn discover_settings_catalog() -> ModSettingsCatalog {
    let mut catalog = ModSettingsCatalog::default();
    for directory in mod_directories() {
        let mod_info_path = directory.join("mod.mod_info");
        if let Ok(source) = fs::read_to_string(mod_info_path) {
            if let Ok(info) = serde_json::from_str::<ModInfoIdentity>(&source) {
                if !info.name.trim().is_empty() {
                    let mod_id = if info.mod_id.trim().is_empty() {
                        directory
                            .file_name()
                            .and_then(|value| value.to_str())
                            .unwrap_or_default()
                            .to_owned()
                    } else {
                        info.mod_id
                    };
                    if mod_id.trim().is_empty() {
                        continue;
                    }
                    catalog.visuals.push(ModVisualEntry {
                        directory: directory.clone(),
                        mod_id,
                        name: info.name,
                        version: info.version,
                        dependencies: info.dependencies,
                    });
                }
            }
        }
        let manifest_path = directory.join(SETTINGS_MANIFEST_FILE);
        let Ok(source) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_str::<ModSettingsManifest>(&source) else {
            continue;
        };
        let expected_mod_id = catalog
            .visuals
            .iter()
            .find(|visual| visual.directory == directory)
            .map(|visual| visual.mod_id.as_str());
        if manifest.schema_version != 1
            || !is_safe_mod_id(&manifest.mod_id)
            || expected_mod_id != Some(manifest.mod_id.as_str())
            || manifest.mod_id.trim().is_empty()
            || manifest.name.trim().is_empty()
            || manifest.controls.is_empty()
            || manifest.controls.len() > 7
            || manifest.controls.iter().any(|control| {
                matches!(control, ModSettingControl::Choice { options, .. } if options.is_empty())
            })
        {
            continue;
        }
        catalog.entries.push(SettingsManifestEntry {
            directory,
            manifest,
        });
    }
    refresh_dependency_cache(&mut catalog);
    catalog
}

fn mod_directories() -> Vec<PathBuf> {
    let Some(game_dir) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    let mut directories = child_directories(&game_dir.join("mods"));
    if let Some(steamapps) = game_dir.parent().and_then(Path::parent) {
        directories.extend(child_directories(
            &steamapps.join("workshop").join("content").join("3009300"),
        ));
    }
    directories
}

fn child_directories(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

fn visual_asset_source(
    catalog: &ModSettingsCatalog,
    assets: &Assets,
    selected_name: &str,
    file_name: &str,
) -> Option<String> {
    let visual = catalog
        .visuals
        .iter()
        .find(|visual| visual.name == selected_name)?;
    let candidates = if file_name == "banner" {
        [
            ("banner.png", "banner"),
            ("assets/banner.png", "assets/banner"),
        ]
    } else {
        [
            ("thumbnail.png", "thumbnail"),
            ("assets/thumbnail.png", "assets/thumbnail"),
        ]
    };
    for (relative, asset_suffix) in candidates {
        if visual.directory.join(relative).is_file() {
            let expected = format!("asset/{}/{}", visual.mod_id, asset_suffix);
            if assets.get_raw(&expected).is_some() {
                return Some(expected);
            }
            let expected_with_extension = format!("{expected}.png");
            if assets.get_raw(&expected_with_extension).is_some() {
                return Some(expected_with_extension);
            }
            let normalized_suffix = format!("/{asset_suffix}");
            if let Some((name, _)) = assets.get_all_assets().into_iter().find(|(name, _)| {
                name.contains(&visual.mod_id)
                    && (name.ends_with(&normalized_suffix)
                        || name.ends_with(&format!("{normalized_suffix}.png")))
            }) {
                return Some(name.clone());
            }
        }
    }
    None
}

fn game_data_dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("TeamSamoyed")
            .join("TeamfightManager2")
            .join("data"),
    )
}

fn safe_relative_path(base: &Path, relative: &str) -> Option<PathBuf> {
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    Some(base.join(relative))
}

fn is_safe_mod_id(value: &str) -> bool {
    let mut components = Path::new(value).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
}

fn settings_storage_dir(entry: &SettingsManifestEntry) -> Option<PathBuf> {
    match entry.manifest.storage.as_str() {
        "mod" => Some(entry.directory.clone()),
        "game_data" => Some(game_data_dir()?.join(&entry.manifest.mod_id)),
        _ => None,
    }
}

fn settings_file_path(entry: &SettingsManifestEntry) -> Option<PathBuf> {
    safe_relative_path(&settings_storage_dir(entry)?, &entry.manifest.settings_file)
}

fn actions_file_path(entry: &SettingsManifestEntry) -> Option<PathBuf> {
    safe_relative_path(&settings_storage_dir(entry)?, &entry.manifest.actions_file)
}

fn load_settings_values(entry: &SettingsManifestEntry) -> JsonMap<String, JsonValue> {
    let Some(path) = settings_file_path(entry) else {
        return JsonMap::new();
    };
    fs::read_to_string(path)
        .ok()
        .and_then(|source| serde_json::from_str::<JsonValue>(&source).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

fn file_card_paths(directory: &str, contains: &str, extension: &str, limit: usize) -> Vec<PathBuf> {
    let Some(root) = game_data_dir() else {
        return Vec::new();
    };
    let Some(directory) = safe_relative_path(&root, directory) else {
        return Vec::new();
    };
    let mut files: Vec<_> = fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !path.is_file()
                || (!contains.is_empty() && !name.contains(contains))
                || (!extension.is_empty()
                    && path.extension().and_then(|value| value.to_str()) != Some(extension))
            {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect();
    files.sort_unstable_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    files
        .into_iter()
        .take(limit.min(5))
        .map(|(_, path)| path)
        .collect()
}

fn file_card_label(path: &Path) -> String {
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("-");
    let stamp = name.split("__before_mod_load_").nth(1).unwrap_or(name);
    let bytes = stamp.as_bytes();
    if bytes.len() >= 13
        && bytes[..8].iter().all(u8::is_ascii_digit)
        && bytes[8] == b'_'
        && bytes[9..13].iter().all(u8::is_ascii_digit)
    {
        let month = match &stamp[4..6] {
            "01" => "Jan",
            "02" => "Feb",
            "03" => "Mar",
            "04" => "Apr",
            "05" => "May",
            "06" => "Jun",
            "07" => "Jul",
            "08" => "Aug",
            "09" => "Sep",
            "10" => "Oct",
            "11" => "Nov",
            "12" => "Dec",
            _ => "",
        };
        if !month.is_empty() {
            return format!(
                "{} {month}\n{}:{}",
                &stamp[6..8],
                &stamp[9..11],
                &stamp[11..13]
            );
        }
    }
    name.chars().take(18).collect()
}

fn write_json_object(path: &Path, values: &JsonMap<String, JsonValue>) -> Result<(), String> {
    let Some(directory) = path.parent() else {
        return Err("invalid settings path".to_owned());
    };
    fs::create_dir_all(directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let source = serde_json::to_string_pretty(&JsonValue::Object(values.clone()))
        .map_err(|error| format!("cannot serialize settings: {error}"))?;
    fs::write(path, format!("{source}\n"))
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn json_values_equal(left: &JsonValue, right: &JsonValue) -> bool {
    left == right
}

fn control_category(control: &ModSettingControl) -> &str {
    match control {
        ModSettingControl::Toggle { category, .. }
        | ModSettingControl::Choice { category, .. }
        | ModSettingControl::Button { category, .. }
        | ModSettingControl::FileCards { category, .. } => category.trim(),
    }
}

fn setting_row_index(row: &Node) -> Option<usize> {
    row.id
        .strip_prefix("owned_setting_row_")
        .and_then(|value| value.parse().ok())
}

fn enabled_mod_ids() -> Vec<String> {
    let Some(game_dir) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    else {
        return Vec::new();
    };
    fs::read_to_string(game_dir.join("config").join("game").join("mods.json"))
        .ok()
        .and_then(|source| serde_json::from_str::<JsonValue>(&source).ok())
        .and_then(|value| value.get("enabled_mods")?.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect()
}

fn numeric_version(value: &str) -> Vec<u32> {
    value
        .trim()
        .trim_start_matches('v')
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

fn compare_versions(left: &str, right: &str) -> std::cmp::Ordering {
    let mut left = numeric_version(left);
    let mut right = numeric_version(right);
    let length = left.len().max(right.len()).max(1);
    left.resize(length, 0);
    right.resize(length, 0);
    left.cmp(&right)
}

fn version_satisfies(version: &str, requirement: &str) -> bool {
    if version.trim().is_empty() || requirement.trim().is_empty() {
        return true;
    }
    requirement.split(',').all(|clause| {
        let clause = clause.trim();
        let (operator, expected) = [">=", "<=", ">", "<", "="]
            .into_iter()
            .find_map(|operator| {
                clause
                    .strip_prefix(operator)
                    .map(|value| (operator, value.trim()))
            })
            .unwrap_or(("=", clause));
        let ordering = compare_versions(version, expected);
        match operator {
            ">=" => ordering.is_ge(),
            "<=" => ordering.is_le(),
            ">" => ordering.is_gt(),
            "<" => ordering.is_lt(),
            _ => ordering.is_eq(),
        }
    })
}

struct DependencyHealth {
    warning: bool,
    tooltip: String,
}

fn dependency_health(catalog: &ModSettingsCatalog, selected_name: &str) -> DependencyHealth {
    dependency_health_with_enabled(catalog, selected_name, &catalog.enabled_mod_ids)
}

fn dependency_health_with_enabled(
    catalog: &ModSettingsCatalog,
    selected_name: &str,
    enabled: &[String],
) -> DependencyHealth {
    let Some(selected) = catalog
        .visuals
        .iter()
        .find(|visual| visual.name == selected_name)
    else {
        return DependencyHealth {
            warning: false,
            tooltip: String::new(),
        };
    };
    if selected.dependencies.is_empty() {
        return DependencyHealth {
            warning: false,
            tooltip: "This mod has no declared dependencies.".to_owned(),
        };
    }
    let mut issues = Vec::new();
    for dependency in &selected.dependencies {
        let installed_version = if dependency.mod_id == "base" {
            Some(GAME_VERSION)
        } else {
            catalog
                .visuals
                .iter()
                .find(|visual| visual.mod_id == dependency.mod_id)
                .map(|visual| visual.version.as_str())
        };
        let Some(installed_version) = installed_version else {
            issues.push(format!("Missing dependency: {}", dependency.mod_id));
            continue;
        };
        if dependency.mod_id != "base" && !enabled.iter().any(|id| id == &dependency.mod_id) {
            issues.push(format!("Dependency disabled: {}", dependency.mod_id));
        }
        if !version_satisfies(installed_version, &dependency.version) {
            issues.push(format!(
                "{} {} does not satisfy {}",
                dependency.mod_id, installed_version, dependency.version
            ));
        }
    }
    if issues.is_empty() {
        DependencyHealth {
            warning: false,
            tooltip: "Dependencies are installed, enabled, and compatible.".to_owned(),
        }
    } else {
        DependencyHealth {
            warning: true,
            tooltip: issues.join("  |  "),
        }
    }
}

fn dependency_warning_entries(
    catalog: &ModSettingsCatalog,
    enabled: &[String],
) -> Vec<(String, String)> {
    catalog
        .visuals
        .iter()
        .filter_map(|visual| {
            let health = dependency_health_with_enabled(catalog, &visual.name, enabled);
            health
                .warning
                .then(|| (visual.name.clone(), health.tooltip))
        })
        .collect()
}

fn refresh_dependency_cache(catalog: &mut ModSettingsCatalog) {
    let enabled = enabled_mod_ids();
    catalog.dependency_warnings = dependency_warning_entries(catalog, &enabled);
    catalog.enabled_mod_ids = enabled;
}

fn sync_mod_settings(root: &mut Node, assets: &Assets, catalog: &mut ModSettingsCatalog) {
    let selected_name = selected_mod_name(root, assets)
        .filter(|name| name != NO_SELECTION)
        .unwrap_or_default();
    let health = dependency_health(catalog, &selected_name);
    catalog.dependency_tooltip = health.tooltip;
    let _ = set_node_visible(root, "owned_dependency_warning", health.warning);
    let selection_changed = catalog.active_name != selected_name;
    if selection_changed {
        catalog.active_name = selected_name.clone();
        catalog.active_entry = catalog
            .entries
            .iter()
            .position(|entry| entry.manifest.name == selected_name);
        catalog.active_thumbnail = None;
        catalog.active_banner = None;
        catalog.values = catalog
            .active_entry
            .and_then(|index| catalog.entries.get(index))
            .map(load_settings_values)
            .unwrap_or_default();
        catalog.status.clear();
        catalog.status_row = None;
        catalog.status_frames = 0;
        catalog.poll_tick = 0;
        catalog.show_settings = false;
        if let Some(rows) = find_node_mut(root, "owned_settings_rows") {
            rows.child.clear();
            rows.runner.set_dirty(true);
        }
    } else {
        catalog.poll_tick = catalog.poll_tick.wrapping_add(1);
        if catalog.status_frames > 0 {
            catalog.status_frames -= 1;
            if catalog.status_frames == 0 {
                catalog.status.clear();
                catalog.status_row = None;
            }
        }
        if catalog.poll_tick.is_multiple_of(60) {
            if let Some(entry) = catalog
                .active_entry
                .and_then(|index| catalog.entries.get(index))
            {
                catalog.values = load_settings_values(entry);
            }
        }
    }

    if selection_changed {
        catalog.active_thumbnail =
            visual_asset_source(catalog, assets, &selected_name, "thumbnail");
        catalog.active_banner = visual_asset_source(catalog, assets, &selected_name, "banner");
    }
    let thumbnail = catalog.active_thumbnail.clone();
    if let Some(source) = thumbnail.as_deref() {
        let _ = set_image_source(root, assets, "owned_thumbnail", source);
    }
    let _ = set_node_visible(root, "owned_thumbnail", thumbnail.is_some());
    let _ = set_node_visible(root, "owned_thumbnail_fallback", thumbnail.is_none());

    let banner = catalog.active_banner.clone();
    if let Some(source) = banner.as_deref() {
        let _ = set_image_source(root, assets, "owned_banner", source);
    }
    let _ = set_node_visible(
        root,
        "owned_banner",
        banner.is_some() && !catalog.show_settings,
    );

    let Some(entry) = catalog
        .active_entry
        .and_then(|index| catalog.entries.get(index))
        .cloned()
    else {
        let _ = set_node_visible(root, "owned_settings_panel", false);
        let _ = set_node_visible(root, "owned_preview_tab_overview", false);
        let _ = set_node_visible(root, "owned_preview_tab_settings", false);
        let _ = set_node_visible(root, "owned_overview_icon", true);
        let _ = set_node_visible(root, "owned_overview_title", true);
        let _ = set_node_visible(root, "owned_description_scroll", true);
        return;
    };

    let _ = set_node_visible(root, "owned_preview_tab_overview", true);
    let _ = set_node_visible(root, "owned_preview_tab_settings", true);
    if let Some(title) = entry
        .manifest
        .display
        .title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = set_label_text(root, "owned_mod_name", title);
    }
    if let Some(author) = entry
        .manifest
        .display
        .author
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = set_label_text(root, "owned_author", author);
    }
    if let Some(version) = entry
        .manifest
        .display
        .version
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = set_label_text(root, "owned_version", &format!("Version {version}"));
    }
    if let Some(source) = entry
        .manifest
        .display
        .source
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let _ = set_label_text(root, "owned_source", source);
    }
    if let Some(dependencies) = entry.manifest.display.dependencies.as_ref() {
        let text = if dependencies.is_empty() {
            "No dependencies".to_owned()
        } else {
            dependencies.join("  |  ")
        };
        let _ = set_label_text(root, "owned_dependencies", &text);
    }
    set_runtime_filter_style(
        root,
        "owned_preview_tab_overview",
        !catalog.show_settings,
        false,
    );
    set_runtime_filter_style(
        root,
        "owned_preview_tab_settings",
        catalog.show_settings,
        false,
    );
    let _ = set_node_visible(root, "owned_settings_panel", catalog.show_settings);
    let _ = set_node_visible(root, "owned_overview_icon", !catalog.show_settings);
    let _ = set_node_visible(root, "owned_overview_title", !catalog.show_settings);
    let _ = set_node_visible(root, "owned_description_scroll", !catalog.show_settings);
    let _ = set_node_visible(
        root,
        "owned_banner",
        banner.is_some() && !catalog.show_settings,
    );
    if !catalog.show_settings {
        let summary = entry
            .manifest
            .display
            .summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(entry.manifest.summary.as_str());
        if !summary.trim().is_empty() {
            let _ = set_label_text(root, "owned_description", summary);
        }
        return;
    }
    let _ = set_label_text(root, "owned_settings_title", "Settings");
    let _ = set_label_text(root, "owned_settings_status", &catalog.status);
    let _ = set_node_visible(
        root,
        "owned_settings_status_badge",
        !catalog.status.is_empty() && catalog.status_row.is_none(),
    );
    let (status_border, status_background) = if catalog.status.starts_with("Error:") {
        ([0.878, 0.325, 0.349, 1.0], [0.290, 0.090, 0.110, 1.0])
    } else if catalog.status == "Saved" {
        ([0.216, 0.835, 0.702, 1.0], [0.078, 0.337, 0.282, 1.0])
    } else {
        ([0.882, 0.651, 0.212, 1.0], [0.286, 0.208, 0.063, 1.0])
    };
    set_runtime_color_style(
        root,
        "owned_settings_status_badge",
        status_border,
        status_background,
        1.0,
    );

    let needs_rows =
        find_node(root, "owned_settings_rows").is_some_and(|rows| rows.child.is_empty());
    if needs_rows {
        let Some(rows) = find_node_mut(root, "owned_settings_rows") else {
            return;
        };
        let mut previous_category = String::new();
        for (index, control) in entry.manifest.controls.iter().enumerate() {
            let category = control_category(control);
            if !category.is_empty() && category != previous_category {
                let Some(template) = load_ui_template(
                    assets,
                    SETTING_CATEGORY_ASSET,
                    "mod_setting_category_runtime",
                ) else {
                    return;
                };
                let mut heading = template.load(assets);
                heading.id = format!("owned_setting_category_{index}");
                let _ = set_label_text(&mut heading, "setting_category_label", category);
                mark_tree_dirty(&mut heading);
                rows.add_child(assets, heading);
                previous_category.clear();
                previous_category.push_str(category);
            }
            let (asset, needle) = if matches!(control, ModSettingControl::FileCards { .. }) {
                (SETTING_FILE_CARDS_ASSET, "mod_file_cards_row_runtime")
            } else {
                (SETTING_ROW_ASSET, "mod_setting_row_runtime")
            };
            let Some(template) = load_ui_template(assets, asset, needle) else {
                return;
            };
            let mut row = template.load(assets);
            row.id = format!("owned_setting_row_{index}");
            mark_tree_dirty(&mut row);
            rows.add_child(assets, row);
        }
    }

    let values = catalog.values.clone();
    let Some(rows) = find_node_mut(root, "owned_settings_rows") else {
        return;
    };
    for row in &mut rows.child {
        let Some(index) = setting_row_index(row) else {
            row.visible = true;
            continue;
        };
        let Some(control) = entry.manifest.controls.get(index) else {
            row.visible = false;
            continue;
        };
        row.visible = true;
        let row_id = row.id.clone();
        set_runtime_color_style(
            row,
            &row_id,
            [0.208, 0.220, 0.286, 1.0],
            [0.114, 0.122, 0.173, 1.0],
            1.0,
        );
        let (label, description) = match control {
            ModSettingControl::Toggle {
                label, description, ..
            }
            | ModSettingControl::Choice {
                label, description, ..
            }
            | ModSettingControl::Button {
                label, description, ..
            }
            | ModSettingControl::FileCards {
                label, description, ..
            } => (label.as_str(), description.as_str()),
        };
        let _ = set_label_text(row, "setting_label", label);
        let _ = set_label_text(row, "setting_description", description);
        let show_row_status = catalog.status_row == Some(index) && !catalog.status.is_empty();
        let _ = set_label_text(row, "setting_status_text", &catalog.status);
        let _ = set_node_visible(row, "setting_status_badge", show_row_status);
        set_runtime_color_style(
            row,
            "setting_status_badge",
            [0.216, 0.835, 0.702, 1.0],
            [0.078, 0.337, 0.282, 1.0],
            1.0,
        );
        let _ = set_node_visible(
            row,
            "setting_toggle",
            matches!(control, ModSettingControl::Toggle { .. }),
        );
        let _ = set_node_visible(
            row,
            "setting_choice",
            matches!(control, ModSettingControl::Choice { .. }),
        );
        let _ = set_node_visible(
            row,
            "setting_button",
            matches!(control, ModSettingControl::Button { .. }),
        );
        for id in ["setting_toggle", "setting_choice"] {
            set_runtime_color_style(
                row,
                id,
                [0.290, 0.298, 0.337, 1.0],
                [0.086, 0.090, 0.129, 1.0],
                1.0,
            );
        }

        match control {
            ModSettingControl::Toggle { key, default, .. } => {
                let enabled = values
                    .get(key)
                    .and_then(JsonValue::as_bool)
                    .unwrap_or(*default);
                for (id, active, on_color) in [
                    ("setting_toggle_off", !enabled, false),
                    ("setting_toggle_on", enabled, true),
                ] {
                    let (border, background) = if active {
                        if on_color {
                            ([0.216, 0.835, 0.702, 1.0], [0.078, 0.337, 0.282, 1.0])
                        } else {
                            ([0.878, 0.325, 0.349, 1.0], [0.290, 0.090, 0.110, 1.0])
                        }
                    } else {
                        ([0.290, 0.298, 0.337, 1.0], [0.086, 0.090, 0.129, 1.0])
                    };
                    set_runtime_color_style(row, id, border, background, 1.0);
                }
            }
            ModSettingControl::Choice { key, options, .. } => {
                let current = values.get(key).unwrap_or(&options[0].value);
                let label = options
                    .iter()
                    .find(|option| json_values_equal(&option.value, current))
                    .map(|option| option.label.as_str())
                    .unwrap_or(options[0].label.as_str());
                let _ = set_label_text(row, "setting_choice_value", label);
                for id in ["setting_choice_previous", "setting_choice_next"] {
                    set_runtime_color_style(
                        row,
                        id,
                        [0.290, 0.298, 0.337, 1.0],
                        [0.145, 0.153, 0.208, 1.0],
                        1.0,
                    );
                }
            }
            ModSettingControl::Button { button_label, .. } => {
                let _ = set_label_text(row, "setting_button_label", button_label);
                set_runtime_color_style(
                    row,
                    "setting_button",
                    [0.290, 0.298, 0.337, 1.0],
                    [0.145, 0.153, 0.208, 1.0],
                    1.0,
                );
            }
            ModSettingControl::FileCards {
                directory,
                filename_contains,
                extension,
                limit,
                ..
            } => {
                let files = file_card_paths(directory, filename_contains, extension, *limit);
                for index in 0..5 {
                    let id = format!("setting_file_card_{index}");
                    let visible = index < files.len();
                    let _ = set_node_visible(row, &id, visible);
                    if let Some(path) = files.get(index) {
                        let label_id = format!("setting_file_card_label_{index}");
                        let _ = set_label_text(row, &label_id, &file_card_label(path));
                    }
                    set_runtime_color_style(
                        row,
                        &id,
                        [0.290, 0.298, 0.337, 1.0],
                        [0.145, 0.153, 0.208, 1.0],
                        1.0,
                    );
                }
                let _ = set_node_visible(row, "setting_file_empty", files.is_empty());
            }
        }
    }
    apply_owned_fonts(root);
}

fn setting_control_at_point(root: &Node, x: f32, y: f32) -> Option<(usize, SettingAction)> {
    if node_contains_point(root, "owned_preview_tab_overview", x, y) {
        return Some((0, SettingAction::ShowOverview));
    }
    if node_contains_point(root, "owned_preview_tab_settings", x, y) {
        return Some((0, SettingAction::ShowSettings));
    }
    let rows = find_visible_node(root, "owned_settings_rows", true)?;
    rows.child.iter().find_map(|row| {
        if !row.visible {
            return None;
        }
        let index = setting_row_index(row)?;
        if node_contains_point(row, "setting_toggle", x, y) {
            return Some((index, SettingAction::Toggle));
        }
        if node_contains_point(row, "setting_choice_previous", x, y) {
            return Some((index, SettingAction::Previous));
        }
        if node_contains_point(row, "setting_choice_next", x, y) {
            return Some((index, SettingAction::Next));
        }
        node_contains_point(row, "setting_button", x, y)
            .then_some((index, SettingAction::Run))
            .or_else(|| {
                (0..5).find_map(|file_index| {
                    node_contains_point(row, &format!("setting_file_card_{file_index}"), x, y)
                        .then_some((index, SettingAction::RunIndex(file_index)))
                })
            })
    })
}

fn apply_setting_action(
    catalog: &mut ModSettingsCatalog,
    row_index: usize,
    action: SettingAction,
) -> bool {
    match action {
        SettingAction::ShowOverview => {
            catalog.show_settings = false;
            return true;
        }
        SettingAction::ShowSettings => {
            catalog.show_settings = true;
            return true;
        }
        _ => {}
    }
    let Some(entry) = catalog
        .active_entry
        .and_then(|index| catalog.entries.get(index))
        .cloned()
    else {
        return false;
    };
    let Some(control) = entry.manifest.controls.get(row_index).cloned() else {
        return false;
    };
    let status_next_to_toggle = matches!(&control, ModSettingControl::Toggle { .. });

    let result = match (control, action) {
        (ModSettingControl::Toggle { key, default, .. }, SettingAction::Toggle) => {
            let current = catalog
                .values
                .get(&key)
                .and_then(JsonValue::as_bool)
                .unwrap_or(default);
            catalog.values.insert(key, JsonValue::Bool(!current));
            settings_file_path(&entry)
                .ok_or_else(|| "invalid settings path".to_owned())
                .and_then(|path| write_json_object(&path, &catalog.values))
                .map(|_| "Saved".to_owned())
        }
        (
            ModSettingControl::Choice { key, options, .. },
            SettingAction::Previous | SettingAction::Next,
        ) => {
            let current = catalog.values.get(&key).unwrap_or(&options[0].value);
            let index = options
                .iter()
                .position(|option| json_values_equal(&option.value, current))
                .unwrap_or(0);
            let next = match action {
                SettingAction::Previous => (index + options.len() - 1) % options.len(),
                SettingAction::Next => (index + 1) % options.len(),
                _ => index,
            };
            catalog.values.insert(key, options[next].value.clone());
            settings_file_path(&entry)
                .ok_or_else(|| "invalid settings path".to_owned())
                .and_then(|path| write_json_object(&path, &catalog.values))
                .map(|_| "Saved".to_owned())
        }
        (ModSettingControl::Button { action, .. }, SettingAction::Run) => {
            let mut request = JsonMap::new();
            request.insert("action".to_owned(), JsonValue::String(action));
            actions_file_path(&entry)
                .ok_or_else(|| "invalid actions path".to_owned())
                .and_then(|path| write_json_object(&path, &request))
                .map(|_| "Action requested".to_owned())
        }
        (ModSettingControl::FileCards { action, .. }, SettingAction::RunIndex(index)) => {
            let mut request = JsonMap::new();
            request.insert("action".to_owned(), JsonValue::String(action));
            request.insert("index".to_owned(), JsonValue::from(index));
            actions_file_path(&entry)
                .ok_or_else(|| "invalid actions path".to_owned())
                .and_then(|path| write_json_object(&path, &request))
                .map(|_| "Import requested".to_owned())
        }
        _ => return false,
    };
    let failed = result.is_err();
    catalog.status = result.unwrap_or_else(|error| format!("Error: {error}"));
    catalog.status_row = if status_next_to_toggle && !failed {
        Some(row_index)
    } else {
        None
    };
    catalog.status_frames = SETTINGS_STATUS_FRAMES;
    true
}

fn mods_popup(root: &Node) -> Option<&Node> {
    find_node(root, "mods_popup")
}

fn mod_rows(root: &Node) -> Option<&[Node]> {
    let popup = mods_popup(root)?;
    let left_panel = find_node(popup, "left_panel")?;
    let table = find_node(left_panel, "table")?;
    let scroll = table.child.iter().find(|child| child.id == "contents")?;
    let contents = scroll.child.iter().find(|child| child.id == "contents")?;
    Some(&contents.child)
}

fn mod_rows_mut(root: &mut Node) -> Option<&mut Vec<Node>> {
    let popup = find_node_mut(root, "mods_popup")?;
    let left_panel = find_node_mut(popup, "left_panel")?;
    let table = find_node_mut(left_panel, "table")?;
    let scroll = table
        .child
        .iter_mut()
        .find(|child| child.id == "contents")?;
    let contents = scroll
        .child
        .iter_mut()
        .find(|child| child.id == "contents")?;
    Some(&mut contents.child)
}

fn mod_row_count(root: &Node) -> usize {
    mod_rows(root).map_or(0, <[Node]>::len)
}

fn mod_visible_indices(root: &Node) -> Vec<usize> {
    mod_rows(root)
        .map(|rows| {
            rows.iter()
                .enumerate()
                .filter_map(|(index, row)| row.visible.then_some(index))
                .collect()
        })
        .unwrap_or_default()
}

fn label_text(node: &Node, assets: &Assets) -> Option<String> {
    if let Some(label) = node.runner_as::<LabelRunner>() {
        return label.rendered_text(assets);
    }
    if node.runner.type_name() != LABEL_RUNNER_TYPE {
        return None;
    }
    let label = unsafe { &*(node.runner.as_ref() as *const dyn NodeRunner as *const LabelRunner) };
    label.rendered_text(assets)
}

fn label_font_name(node: &Node) -> Option<String> {
    if node.runner.type_name() != LABEL_RUNNER_TYPE {
        return None;
    }
    let label = unsafe { &*(node.runner.as_ref() as *const dyn NodeRunner as *const LabelRunner) };
    Some(label.style.normal.font.clone())
}

fn apply_owned_font_tree(node: &mut Node, regular: &str, bold: &str, bold_context: bool) {
    let owns_bold_children = node.id.starts_with("mod_menu_filter_")
        || matches!(
            node.id.as_str(),
            "owned_workshop_button" | "owned_close_button" | "owned_enabled" | "owned_disabled"
        );
    let is_bold = bold_context
        || owns_bold_children
        || matches!(
            node.id.as_str(),
            "owned_title" | "owned_mod_name" | "owned_version" | "owned_overview_title"
        );
    if node.runner.type_name() == LABEL_RUNNER_TYPE {
        let label =
            unsafe { &mut *(node.runner.as_mut() as *mut dyn NodeRunner as *mut LabelRunner) };
        let font = if is_bold { bold } else { regular };
        let mut changed = false;
        for property in [
            &mut label.style.normal,
            &mut label.style.hover,
            &mut label.style.active,
            &mut label.style.disabled,
        ] {
            if property.font != font {
                property.font = font.to_owned();
                changed = true;
            }
        }
        if changed {
            label.set_dirty(true);
        }
    }
    for child in &mut node.child {
        apply_owned_font_tree(child, regular, bold, is_bold);
    }
}

fn apply_owned_fonts(root: &mut Node) {
    let Some(popup) = find_node_mut(root, "mods_popup") else {
        return;
    };
    let regular = find_node(popup, "left_panel")
        .and_then(|panel| find_node(panel, "table_header"))
        .and_then(|header| find_node(header, "name"))
        .and_then(label_font_name)
        .unwrap_or_else(|| "asset/base/font/set/regular".to_owned());
    let bold = find_node(popup, "header")
        .and_then(|header| find_node(header, "title"))
        .and_then(label_font_name)
        .unwrap_or_else(|| "asset/base/font/set/bold".to_owned());
    if let Some(surface) = find_node_mut(popup, "better_mod_menu_surface") {
        apply_owned_font_tree(surface, &regular, &bold, false);
    }
}

fn selected_mod_name(root: &Node, assets: &Assets) -> Option<String> {
    label_text(find_node(mods_popup(root)?, "mod_name")?, assets)
}

fn mod_row_name(row: &Node, assets: &Assets) -> Option<String> {
    label_text(find_node(row, "name")?, assets)
}

fn mod_row_search_text(row: &Node, assets: &Assets) -> String {
    let name = mod_row_name(row, assets).unwrap_or_default();
    let author = find_node(row, "author")
        .and_then(|node| label_text(node, assets))
        .unwrap_or_default();
    format!("{name} {author}").to_lowercase()
}

fn mod_row_index_by_name(root: &Node, assets: &Assets, name: &str) -> Option<usize> {
    mod_rows(root)?
        .iter()
        .position(|row| mod_row_name(row, assets).is_some_and(|row_name| row_name == name))
}

fn apply_mod_filter(root: &mut Node, assets: &Assets, query: &str, filter: usize) -> usize {
    let query = query.trim().to_lowercase();
    let visibility: Vec<bool> = mod_rows(root)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let matches_query =
                        query.is_empty() || mod_row_search_text(row, assets).contains(&query);
                    let matches_source = match filter {
                        FILTER_CODE => {
                            find_node(row, "code_badge").is_some_and(|badge| badge.visible)
                        }
                        FILTER_WORKSHOP => {
                            find_node(row, "workshop_badge").is_some_and(|badge| badge.visible)
                        }
                        _ => true,
                    };
                    matches_query && matches_source
                })
                .collect()
        })
        .unwrap_or_default();

    if let Some(rows) = mod_rows_mut(root) {
        for (row, visible) in rows.iter_mut().zip(visibility.iter().copied()) {
            row.visible = visible;
        }
    }
    visibility.into_iter().filter(|visible| *visible).count()
}

fn node_center(node: &Node) -> Option<(f32, f32)> {
    (node.rect.w > 0.0 && node.rect.h > 0.0).then_some((
        node.rect.x + node.rect.w * 0.5,
        node.rect.y + node.rect.h * 0.5,
    ))
}

fn mod_row_click_point(root: &Node, index: usize) -> Option<(f32, f32)> {
    node_center(mod_rows(root)?.get(index)?)
}

fn mod_row_control_click_point(root: &Node, index: usize, control_id: &str) -> Option<(f32, f32)> {
    node_center(find_node(mod_rows(root)?.get(index)?, control_id)?)
}

fn mod_row_toggle_at_point(
    root: &Node,
    assets: &Assets,
    x: f32,
    y: f32,
) -> Option<(usize, String)> {
    mod_rows(root)?.iter().enumerate().find_map(|(index, row)| {
        if !row.visible
            || (!node_contains_point(row, "enabled", x, y)
                && !node_contains_point(row, "disabled", x, y))
        {
            return None;
        }
        mod_row_name(row, assets).map(|name| (index, name))
    })
}

fn mark_tree_dirty(node: &mut Node) {
    node.runner.set_dirty(true);
    for child in &mut node.child {
        mark_tree_dirty(child);
    }
}

fn copy_layout(target: &mut Node, donor: &Node) {
    target.layout = donor.layout.clone();
}

fn copy_color_style(target: &mut Node, donor: &Node) {
    let Some(style) = donor
        .runner_as::<ColorRunner>()
        .map(|runner| runner.style.clone())
    else {
        return;
    };
    if let Some(runner) = target.runner_as_mut::<ColorRunner>() {
        runner.style = style;
    }
}

fn apply_base_layout(popup: &mut Node, blueprint: &Node) {
    if let (Some(target), Some(donor)) = (
        find_node_mut(popup, "header"),
        find_node(blueprint, "bmm_donor_header"),
    ) {
        copy_layout(target, donor);
    }
    if let Some(header) = find_node_mut(popup, "header") {
        if let (Some(target), Some(donor)) = (
            find_node_mut(header, "title"),
            find_node(blueprint, "bmm_donor_title"),
        ) {
            copy_layout(target, donor);
        }
    }

    if let Some(left_panel) = find_node_mut(popup, "left_panel") {
        if let (Some(table), Some(donor)) = (
            find_node_mut(left_panel, "table"),
            find_node(blueprint, "bmm_donor_table"),
        ) {
            copy_layout(table, donor);
        }
        if let Some(table) = find_node_mut(left_panel, "table") {
            if let (Some(header), Some(donor)) = (
                find_node_mut(table, "table_header"),
                find_node(blueprint, "bmm_donor_table_header"),
            ) {
                copy_layout(header, donor);
            }
            if let Some(header) = find_node_mut(table, "table_header") {
                for (target_id, donor_id) in [
                    ("name", "bmm_donor_table_name"),
                    ("author", "bmm_donor_table_author"),
                    ("active", "bmm_donor_table_status"),
                ] {
                    if let (Some(target), Some(donor)) = (
                        find_node_mut(header, target_id),
                        find_node(blueprint, donor_id),
                    ) {
                        copy_layout(target, donor);
                    }
                }
            }
            if let (Some(scroll), Some(donor)) = (
                table.child.iter_mut().find(|node| node.id == "contents"),
                find_node(blueprint, "bmm_donor_table_scroll"),
            ) {
                copy_layout(scroll, donor);
            }
        }
    }

    if let Some(right_panel) = find_node_mut(popup, "right_panel") {
        if let Some(top) = find_node_mut(right_panel, "top") {
            for (target_id, donor_id) in [
                ("thumbnail_panel", "bmm_donor_thumbnail_panel"),
                ("info_panel", "bmm_donor_info_panel"),
            ] {
                if let (Some(target), Some(donor)) = (
                    find_node_mut(top, target_id),
                    find_node(blueprint, donor_id),
                ) {
                    copy_layout(target, donor);
                    copy_color_style(target, donor);
                }
            }
            if let Some(info) = find_node_mut(top, "info_panel") {
                for (target_id, donor_id) in [
                    ("mod_name", "bmm_donor_mod_name"),
                    ("mod_author", "bmm_donor_mod_author"),
                    ("mod_date", "bmm_donor_mod_date"),
                ] {
                    if let (Some(target), Some(donor)) = (
                        find_node_mut(info, target_id),
                        find_node(blueprint, donor_id),
                    ) {
                        copy_layout(target, donor);
                    }
                }
            }
        }

        if let (Some(description), Some(donor)) = (
            find_node_mut(right_panel, "description_panel"),
            find_node(blueprint, "bmm_donor_description_panel"),
        ) {
            copy_layout(description, donor);
            copy_color_style(description, donor);
        }
        if let Some(description) = find_node_mut(right_panel, "description_panel") {
            if let (Some(scroll), Some(donor)) = (
                find_node_mut(description, "description_scroll"),
                find_node(blueprint, "bmm_donor_description_scroll"),
            ) {
                copy_layout(scroll, donor);
            }
        }
    }

    if let (Some(buttons), Some(donor)) = (
        find_node_mut(popup, "bottom_buttons"),
        find_node(blueprint, "bmm_donor_bottom_buttons"),
    ) {
        copy_layout(buttons, donor);
    }
}

fn load_ui_template(assets: &Assets, preferred: &str, needle: &str) -> Option<NodeTemplate> {
    if let Some(template) = assets.get::<NodeTemplate>(preferred) {
        return Some(template.clone());
    }
    let mut candidates = vec![preferred.to_owned()];
    candidates.extend(
        assets
            .get_all_assets()
            .into_iter()
            .filter(|(name, _)| name.contains(needle) && name.as_str() != preferred)
            .map(|(name, _)| name.clone()),
    );
    for candidate in candidates {
        if let Some(template) = assets.get::<NodeTemplate>(&candidate) {
            return Some(template.clone());
        }
        let raw = assets.get_raw(&candidate)?;
        let bytes = raw
            .merge_raw
            .clone()
            .or_else(|| raw.file_path.as_ref().and_then(|path| fs::read(path).ok()))?;
        let source = std::str::from_utf8(&bytes).ok()?;
        if let Ok((remaining, template)) = parse_node_template(source) {
            if remaining.trim().is_empty() {
                return Some(template);
            }
        }
    }
    None
}

fn set_runtime_color_style(
    node: &mut Node,
    id: &str,
    border: [f32; 4],
    background: [f32; 4],
    stroke: f32,
) {
    let Some(target) = find_node_mut(node, id) else {
        return;
    };
    if target.runner.type_name() != COLOR_RUNNER_TYPE {
        return;
    }
    let runner =
        unsafe { &mut *(target.runner.as_mut() as *mut dyn NodeRunner as *mut ColorRunner) };
    for property in [
        &mut runner.style.normal,
        &mut runner.style.hover,
        &mut runner.style.active,
        &mut runner.style.disabled,
    ] {
        property.color.r = border[0];
        property.color.g = border[1];
        property.color.b = border[2];
        property.color.a = border[3];
        property.stroke = stroke;
        if let Some(back) = property.back_color.as_mut() {
            back.r = background[0];
            back.g = background[1];
            back.b = background[2];
            back.a = background[3];
        }
    }
    runner.set_dirty(true);
}

#[allow(clippy::approx_constant)]
fn set_runtime_filter_style(node: &mut Node, id: &str, active: bool, hovered: bool) {
    let (border, background) = match (active, hovered) {
        (true, true) => ([0.290, 0.890, 0.755, 1.0], [0.094, 0.380, 0.318, 1.0]),
        (true, false) => ([0.216, 0.835, 0.702, 1.0], [0.078, 0.337, 0.282, 1.0]),
        (false, true) => ([0.345, 0.365, 0.431, 1.0], [0.153, 0.161, 0.216, 1.0]),
        (false, false) => ([0.290, 0.298, 0.337, 1.0], [0.114, 0.122, 0.173, 1.0]),
    };
    set_runtime_color_style(node, id, border, background, 1.0);
}

#[allow(clippy::approx_constant)]
fn set_runtime_search_style(node: &mut Node) {
    let Some(search) = find_node_mut(node, "mod_menu_search") else {
        return;
    };
    if search.runner.type_name() != TEXT_EDIT_RUNNER_TYPE {
        return;
    }
    let runner =
        unsafe { &mut *(search.runner.as_mut() as *mut dyn NodeRunner as *mut TextEditRunner) };
    for property in [
        &mut runner.style.normal,
        &mut runner.style.hover,
        &mut runner.style.active,
        &mut runner.style.disabled,
    ] {
        property.color.r = 0.290;
        property.color.g = 0.298;
        property.color.b = 0.337;
        property.color.a = 1.0;
        if let Some(back) = property.back_color.as_mut() {
            back.r = 0.114;
            back.g = 0.122;
            back.b = 0.173;
            back.a = 1.0;
        }
        property.text_color.r = 0.910;
        property.text_color.g = 0.910;
        property.text_color.b = 0.910;
        property.text_color.a = 1.0;
        property.placeholder_color.r = 0.522;
        property.placeholder_color.g = 0.553;
        property.placeholder_color.b = 0.616;
        property.placeholder_color.a = 1.0;
        property.selection_color.r = 0.090;
        property.selection_color.g = 0.392;
        property.selection_color.b = 0.318;
        property.selection_color.a = 1.0;
    }
    runner.set_dirty(true);
}

fn tune_runtime_styles(popup: &mut Node) {
    set_runtime_color_style(
        popup,
        "better_mod_menu_surface",
        [0.086, 0.090, 0.129, 1.0],
        [0.086, 0.090, 0.129, 1.0],
        0.0,
    );
    set_runtime_color_style(
        popup,
        "owned_table_header",
        [0.086, 0.090, 0.129, 1.0],
        [0.086, 0.090, 0.129, 1.0],
        0.0,
    );
    for id in [
        "owned_thumbnail_card",
        "owned_info_card",
        "owned_preview_card",
    ] {
        set_runtime_color_style(
            popup,
            id,
            [0.208, 0.220, 0.286, 1.0],
            [0.125, 0.133, 0.188, 1.0],
            1.0,
        );
    }
    for id in ["owned_workshop_button", "owned_close_button"] {
        set_runtime_color_style(
            popup,
            id,
            [0.290, 0.298, 0.337, 1.0],
            [0.114, 0.122, 0.173, 1.0],
            1.0,
        );
    }
    set_runtime_color_style(
        popup,
        "owned_tooltip",
        [0.290, 0.298, 0.337, 1.0],
        [0.086, 0.090, 0.129, 1.0],
        1.0,
    );
    set_runtime_search_style(popup);
    for id in [
        "mod_menu_filter_all",
        "mod_menu_filter_code",
        "mod_menu_filter_workshop",
    ] {
        set_runtime_filter_style(popup, id, false, false);
    }
    for id in [
        "mod_menu_filter_all_active",
        "mod_menu_filter_code_active",
        "mod_menu_filter_workshop_active",
    ] {
        set_runtime_filter_style(popup, id, true, false);
    }
}

fn ensure_runtime_ui(root: &mut Node, assets: &Assets) -> bool {
    if find_node(root, "better_mod_menu_surface").is_some() {
        return true;
    }
    let Some(template) = load_ui_template(assets, RUNTIME_LAYOUT_ASSET, "better_mod_menu_runtime")
    else {
        if let Some(popup) = find_node_mut(root, "mods_popup") {
            let matches = assets
                .get_all_assets()
                .into_iter()
                .filter(|(name, _)| name.contains("mod_menu"))
                .count();
            let _ = set_label_text(
                popup,
                "title",
                &format!("Mod Manager · runtime asset unavailable ({matches})"),
            );
        }
        return false;
    };
    let mut surface = template.load(assets);
    mark_tree_dirty(&mut surface);

    let Some(popup) = find_node_mut(root, "mods_popup") else {
        return false;
    };
    apply_base_layout(popup, &surface);
    popup.add_child(assets, surface);
    tune_runtime_styles(popup);
    mark_tree_dirty(popup);
    true
}

fn copy_row_layout_tree(target: &mut Node, donor: &Node) {
    target.layout = donor.layout.clone();
    copy_color_style(target, donor);

    for child in &mut target.child {
        if let Some(donor_child) = find_node(donor, &child.id) {
            copy_row_layout_tree(child, donor_child);
        }
    }
}

fn apply_mod_row_layouts(root: &mut Node, assets: &Assets) {
    let Some(template) = load_ui_template(assets, RUNTIME_SLOT_ASSET, "mod_slot_runtime") else {
        return;
    };
    let donor = template.load(assets);
    let Some(rows) = mod_rows_mut(root) else {
        return;
    };
    for row in rows {
        let row_id = row.id.clone();
        copy_row_layout_tree(row, &donor);
        row.id = row_id;
    }
}

fn color_selectable_selected(node: &Node) -> Option<bool> {
    if node.runner.type_name() != COLOR_SELECTABLE_RUNNER_TYPE {
        return None;
    }
    let runner = unsafe {
        &*(node.runner.as_ref() as *const dyn NodeRunner as *const ColorSelectableRunner)
    };
    Some(runner.selected)
}

fn current_mod_states(root: &Node, assets: &Assets) -> Vec<(String, bool)> {
    mod_rows(root)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let name = mod_row_name(row, assets)?;
                    let enabled = find_node(row, "enabled")
                        .and_then(color_selectable_selected)
                        .unwrap_or(false);
                    Some((name, enabled))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn row_status(
    name: &str,
    enabled: bool,
    baseline_mod_states: &[(String, bool)],
    dependency_warnings: &[(String, String)],
) -> usize {
    // Update detection will take this highest-priority branch once a trusted
    // update source is available.
    let update_available = false;
    if update_available {
        STATUS_UPDATE
    } else if dependency_warnings
        .iter()
        .any(|(warning_name, _)| warning_name == name)
    {
        STATUS_WARNING
    } else if baseline_mod_states
        .iter()
        .find(|(baseline_name, _)| baseline_name == name)
        .is_some_and(|(_, baseline_enabled)| *baseline_enabled != enabled)
    {
        STATUS_RESTART
    } else {
        STATUS_NONE
    }
}

fn update_toggle_visual(
    transitions: &mut Vec<ToggleTransition>,
    name: &str,
    enabled: bool,
) -> ToggleVisual {
    let Some(transition) = transitions
        .iter_mut()
        .find(|transition| transition.name == name)
    else {
        transitions.push(ToggleTransition {
            name: name.to_owned(),
            last_enabled: enabled,
            from_enabled: enabled,
            progress: 1.0,
        });
        return ToggleVisual {
            from_enabled: enabled,
            to_enabled: enabled,
            progress: 1.0,
        };
    };

    if transition.last_enabled != enabled {
        transition.from_enabled = transition.last_enabled;
        transition.last_enabled = enabled;
        transition.progress = 0.0;
    } else if transition.progress < 1.0 {
        transition.progress = (transition.progress + 1.0 / TOGGLE_TRANSITION_FRAMES).min(1.0);
    }

    ToggleVisual {
        from_enabled: transition.from_enabled,
        to_enabled: transition.last_enabled,
        progress: transition.progress,
    }
}

fn mix_color(left: [f32; 4], right: [f32; 4], amount: f32) -> [f32; 4] {
    let amount = amount.clamp(0.0, 1.0);
    [
        left[0] + (right[0] - left[0]) * amount,
        left[1] + (right[1] - left[1]) * amount,
        left[2] + (right[2] - left[2]) * amount,
        left[3] + (right[3] - left[3]) * amount,
    ]
}

fn tune_owned_row(row: &mut Node, selected: bool, status: usize, visual: ToggleVisual) {
    let row_id = row.id.clone();
    let (row_border, row_background, row_stroke) = if selected {
        ([0.761, 0.776, 0.808, 1.0], [0.125, 0.133, 0.192, 1.0], 1.0)
    } else {
        ([0.0, 0.0, 0.0, 0.0], [0.114, 0.122, 0.173, 1.0], 0.0)
    };
    set_runtime_color_style(row, &row_id, row_border, row_background, row_stroke);
    set_runtime_color_style(
        row,
        "owned_toggle_base",
        [0.290, 0.298, 0.337, 1.0],
        [0.0, 0.0, 0.0, 0.0],
        1.0,
    );
    let has_status = status != STATUS_NONE;
    let _ = set_node_visible(row, "owned_status_slot", has_status);
    let _ = set_node_visible(row, "owned_status_restart", status == STATUS_RESTART);
    let _ = set_node_visible(row, "owned_status_update", status == STATUS_UPDATE);
    let _ = set_node_visible(row, "owned_status_warning", status == STATUS_WARNING);
    if has_status {
        let (border, background) = match status {
            STATUS_UPDATE => ([0.118, 0.694, 0.749, 1.0], [0.059, 0.247, 0.286, 1.0]),
            STATUS_WARNING => ([0.780, 0.345, 0.267, 1.0], [0.286, 0.102, 0.094, 1.0]),
            _ => ([0.620, 0.471, 0.184, 1.0], [0.216, 0.169, 0.078, 1.0]),
        };
        set_runtime_color_style(row, "owned_status_slot", border, background, 1.0);
    }
    for id in ["owned_enabled", "owned_disabled"] {
        set_runtime_color_style(row, id, [0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], 0.0);
    }

    let position = if visual.progress >= 1.0 {
        if visual.to_enabled {
            0.0
        } else {
            1.0
        }
    } else if visual.from_enabled {
        visual.progress
    } else {
        1.0 - visual.progress
    };
    let thumb_index = (position * 8.0).round().clamp(0.0, 8.0) as usize;
    for index in 0..9 {
        let _ = set_node_visible(
            row,
            &format!("owned_toggle_thumb_{index}"),
            index == thumb_index,
        );
    }

    let fill_strength = (position * 2.0 - 1.0).abs();
    let teal_border = [0.216, 0.835, 0.702, 1.0];
    let neutral_border = [0.720, 0.780, 0.776, 1.0];
    let red_border = [0.878, 0.325, 0.349, 1.0];
    let teal_fill = [0.078, 0.337, 0.282, fill_strength];
    let neutral_fill = [0.125, 0.133, 0.188, 0.0];
    let red_fill = [0.290, 0.090, 0.110, fill_strength];
    let (border, background) = if position <= 0.5 {
        let amount = position * 2.0;
        (
            mix_color(teal_border, neutral_border, amount),
            mix_color(teal_fill, neutral_fill, amount),
        )
    } else {
        let amount = (position - 0.5) * 2.0;
        (
            mix_color(neutral_border, red_border, amount),
            mix_color(neutral_fill, red_fill, amount),
        )
    };
    set_runtime_color_style(
        row,
        &format!("owned_toggle_thumb_{thumb_index}"),
        border,
        background,
        1.0,
    );
}

fn sync_owned_rows(
    root: &mut Node,
    assets: &Assets,
    selection_hint: &str,
    prefer_selection_hint: bool,
    baseline_mod_states: &[(String, bool)],
    dependency_warnings: &[(String, String)],
    transitions: &mut Vec<ToggleTransition>,
) {
    let count = mod_row_count(root);
    let current_count = find_node(root, "owned_rows").map_or(0, |rows| rows.child.len());
    if current_count < count {
        let Some(template) = load_ui_template(assets, OWNED_ROW_ASSET, "owned_mod_row_runtime")
        else {
            return;
        };
        let Some(rows) = find_node_mut(root, "owned_rows") else {
            return;
        };
        for index in current_count..count {
            let mut row = template.load(assets);
            row.id = format!("owned_mod_row_{index}");
            mark_tree_dirty(&mut row);
            rows.add_child(assets, row);
        }
    }

    let selected_name = if prefer_selection_hint && !selection_hint.is_empty() {
        selection_hint.to_owned()
    } else {
        selected_mod_name(root, assets)
            .filter(|name| name != NO_SELECTION)
            .unwrap_or_else(|| selection_hint.to_owned())
    };
    let data: Vec<(String, String, String, bool, bool, bool, usize)> = mod_rows(root)
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let name = mod_row_name(row, assets).unwrap_or_default();
                    let author = find_node(row, "author")
                        .and_then(|node| label_text(node, assets))
                        .unwrap_or_default();
                    let source =
                        if find_node(row, "workshop_badge").is_some_and(|badge| badge.visible) {
                            "Workshop"
                        } else {
                            "Code"
                        }
                        .to_owned();
                    let enabled = find_node(row, "enabled")
                        .and_then(color_selectable_selected)
                        .unwrap_or(false);
                    let status =
                        row_status(&name, enabled, baseline_mod_states, dependency_warnings);
                    (
                        name.clone(),
                        author,
                        source,
                        row.visible,
                        name == selected_name,
                        enabled,
                        status,
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let data: Vec<_> = data
        .into_iter()
        .map(
            |(name, author, source, visible, selected, enabled, status)| {
                let visual = update_toggle_visual(transitions, &name, enabled);
                (name, author, source, visible, selected, status, visual)
            },
        )
        .collect();

    let Some(rows) = find_node_mut(root, "owned_rows") else {
        return;
    };
    for (row, (name, author, source, visible, selected, status, visual)) in
        rows.child.iter_mut().zip(data)
    {
        row.visible = visible;
        let _ = set_label_text(row, "owned_row_name", &name);
        let _ = set_label_text(row, "owned_row_author", &author);
        let _ = set_label_text(row, "owned_row_source", &source);
        tune_owned_row(row, selected, status, visual);
    }
    apply_owned_fonts(root);
}

fn search_state(root: &Node) -> Option<(String, bool)> {
    let search = find_node(root, "mod_menu_search")?;
    if let Some(runner) = search.runner_as::<TextEditRunner>() {
        return Some((runner.text.clone(), runner.is_editing));
    }
    if search.runner.type_name() != TEXT_EDIT_RUNNER_TYPE {
        return None;
    }
    let runner =
        unsafe { &*(search.runner.as_ref() as *const dyn NodeRunner as *const TextEditRunner) };
    Some((runner.text.clone(), runner.is_editing))
}

fn set_search_text(root: &mut Node, text: &str) -> bool {
    let Some(search) = find_node_mut(root, "mod_menu_search") else {
        return false;
    };
    if let Some(runner) = search.runner_as_mut::<TextEditRunner>() {
        runner.set_text(text);
        runner.set_dirty(true);
        return true;
    }
    if search.runner.type_name() != TEXT_EDIT_RUNNER_TYPE {
        return false;
    }
    let runner =
        unsafe { &mut *(search.runner.as_mut() as *mut dyn NodeRunner as *mut TextEditRunner) };
    runner.set_text(text);
    runner.set_dirty(true);
    true
}

fn set_search_editing(root: &mut Node, editing: bool) -> bool {
    let Some(search) = find_node_mut(root, "mod_menu_search") else {
        return false;
    };
    if let Some(runner) = search.runner_as_mut::<TextEditRunner>() {
        runner.is_editing = editing;
        runner.set_dirty(true);
        return true;
    }
    if search.runner.type_name() != TEXT_EDIT_RUNNER_TYPE {
        return false;
    }
    let runner =
        unsafe { &mut *(search.runner.as_mut() as *mut dyn NodeRunner as *mut TextEditRunner) };
    runner.is_editing = editing;
    runner.set_dirty(true);
    true
}

fn set_label_text(node: &mut Node, id: &str, text: &str) -> bool {
    if node.id == id {
        if let Some(label) = node.runner_as_mut::<LabelRunner>() {
            if label.text != text {
                label.text = text.into();
                label.set_dirty(true);
            }
            return true;
        }
        if node.runner.type_name() != LABEL_RUNNER_TYPE {
            return false;
        }
        let label =
            unsafe { &mut *(node.runner.as_mut() as *mut dyn NodeRunner as *mut LabelRunner) };
        if label.text != text {
            label.text = text.into();
            label.set_dirty(true);
        }
        return true;
    }
    node.child
        .iter_mut()
        .any(|child| set_label_text(child, id, text))
}

fn image_source(root: &Node, source_id: &str) -> Option<String> {
    find_node(root, source_id)
        .and_then(|node| node.runner_as::<ImageRunner>())
        .map(|image| image.style.normal.source.clone())
        .filter(|source| !source.trim().is_empty())
}

fn set_image_source(root: &mut Node, assets: &Assets, target_id: &str, source: &str) -> bool {
    if source.trim().is_empty() {
        return false;
    }
    replace_image_child(root, assets, target_id, source)
}

fn replace_image_child(node: &mut Node, assets: &Assets, target_id: &str, source: &str) -> bool {
    if let Some(index) = node.child.iter().position(|child| child.id == target_id) {
        if image_source(&node.child[index], target_id).as_deref() == Some(source) {
            return true;
        }
        let template_source =
            format!("runtime_image:image {{ source: \"{source}\"; ignore_event: true; }}");
        let Ok((remaining, template)) = parse_node_template(&template_source) else {
            return false;
        };
        if !remaining.trim().is_empty() {
            return false;
        }
        let previous = &node.child[index];
        let mut replacement = template.load(assets);
        replacement.id = target_id.to_owned();
        replacement.layout = previous.layout.clone();
        replacement.visible = previous.visible;
        mark_tree_dirty(&mut replacement);
        node.child[index] = replacement;
        node.runner.set_dirty(true);
        return true;
    }
    node.child
        .iter_mut()
        .any(|child| replace_image_child(child, assets, target_id, source))
}

fn copy_image_source(root: &mut Node, assets: &Assets, source_id: &str, target_id: &str) -> bool {
    let Some(source) = image_source(root, source_id) else {
        return false;
    };
    set_image_source(root, assets, target_id, &source)
}

fn collect_label_texts(node: &Node, assets: &Assets, output: &mut Vec<String>) {
    if let Some(text) = label_text(node, assets) {
        let text = text.trim();
        if !text.is_empty() && text != NO_SELECTION {
            output.push(text.to_owned());
        }
    }
    for child in &node.child {
        collect_label_texts(child, assets, output);
    }
}

fn truncate_preview_text(text: &str, max_chars: usize) -> String {
    let mut characters = text.chars();
    let truncated: String = characters.by_ref().take(max_chars).collect();
    if characters.next().is_some() {
        format!("{}...", truncated.trim_end())
    } else {
        truncated
    }
}

fn sync_selected_preview(root: &mut Node, assets: &Assets) {
    let Some(popup) = mods_popup(root) else {
        return;
    };
    let selected_name = selected_mod_name(root, assets)
        .filter(|name| name != NO_SELECTION)
        .unwrap_or_else(|| "Select a mod".to_owned());
    let has_selection = selected_name != "Select a mod";
    let author = find_node(popup, "mod_author")
        .and_then(|node| label_text(node, assets))
        .filter(|text| !text.trim().is_empty() && text.trim() != NO_SELECTION)
        .unwrap_or_else(|| "-".to_owned());
    let version = find_node(popup, "version_text")
        .and_then(|node| label_text(node, assets))
        .filter(|text| !text.trim().is_empty() && text.trim() != NO_SELECTION)
        .map(|text| format!("Version {}", text.trim()))
        .unwrap_or_else(|| "Version -".to_owned());
    let source = mod_rows(root)
        .and_then(|rows| {
            rows.iter()
                .find(|row| mod_row_name(row, assets).as_deref() == Some(&selected_name))
        })
        .map(|row| {
            if find_node(row, "workshop_badge").is_some_and(|badge| badge.visible) {
                "Workshop"
            } else {
                "Code"
            }
        })
        .unwrap_or("-");
    let description = find_node(popup, "description_text")
        .and_then(|node| label_text(node, assets))
        .filter(|text| !text.trim().is_empty() && text.trim() != NO_SELECTION)
        .unwrap_or_else(|| {
            if has_selection {
                "This mod does not provide a description.".to_owned()
            } else {
                "Choose a mod from the list to inspect its details.".to_owned()
            }
        });

    let mut dependencies = Vec::new();
    if let Some(deps_scroll) = find_node(popup, "deps_scroll") {
        collect_label_texts(deps_scroll, assets, &mut dependencies);
    }
    let dependency_summary = if dependencies.is_empty() {
        "No dependencies".to_owned()
    } else {
        truncate_preview_text(&format!("Requires {}", dependencies.join(" | ")), 46)
    };

    let Some(popup) = find_node_mut(root, "mods_popup") else {
        return;
    };
    let has_thumbnail = copy_image_source(popup, assets, "thumbnail", "owned_thumbnail");
    let _ = set_node_visible(popup, "owned_thumbnail_fallback", !has_thumbnail);
    let _ = set_node_visible(popup, "owned_thumbnail", has_thumbnail);
    let _ = set_label_text(popup, "owned_mod_name", &selected_name);
    let _ = set_label_text(popup, "owned_author", &author);
    let _ = set_label_text(popup, "owned_version", &version);
    let _ = set_label_text(popup, "owned_source", source);
    let _ = set_label_text(popup, "owned_dependencies", &dependency_summary);
    let _ = set_label_text(popup, "owned_description", &description);
    let _ = set_node_visible(popup, "owned_author_icon", has_selection);
    let _ = set_node_visible(popup, "owned_source_icon", has_selection);
    let _ = set_node_visible(popup, "owned_dependency_icon", has_selection);
}

struct MenuChromeState<'a> {
    installed: usize,
    visible: usize,
    enabled: usize,
    query: &'a str,
    filter: usize,
    hovered_filter: Option<usize>,
    pending_restart: bool,
}

fn sync_menu_chrome(root: &mut Node, state: MenuChromeState<'_>) {
    let disabled = state.installed.saturating_sub(state.enabled);
    let summary = if state.visible == state.installed {
        format!(
            "{} installed  |  {} enabled  |  {disabled} disabled",
            state.installed, state.enabled
        )
    } else {
        format!(
            "{} shown  |  {} installed  |  {} enabled  |  {disabled} disabled",
            state.visible, state.installed, state.enabled
        )
    };
    let Some(popup) = find_node_mut(root, "mods_popup") else {
        return;
    };
    let _ = set_label_text(popup, "owned_title", "Better Mod Menu");
    let _ = set_label_text(popup, "owned_summary", &summary);
    let _ = set_node_visible(popup, "mod_menu_search_clear", !state.query.is_empty());
    let _ = set_node_visible(popup, "owned_empty_state", state.visible == 0);
    let _ = set_node_visible(popup, "owned_rows", state.visible > 0);
    let _ = set_node_visible(popup, "owned_restart_button", state.pending_restart);
    set_runtime_color_style(
        popup,
        "owned_restart_button",
        [0.620, 0.471, 0.184, 1.0],
        [0.216, 0.169, 0.078, 1.0],
        1.0,
    );
    for (candidate, normal_id, active_id) in [
        (
            FILTER_ALL,
            "mod_menu_filter_all",
            "mod_menu_filter_all_active",
        ),
        (
            FILTER_CODE,
            "mod_menu_filter_code",
            "mod_menu_filter_code_active",
        ),
        (
            FILTER_WORKSHOP,
            "mod_menu_filter_workshop",
            "mod_menu_filter_workshop_active",
        ),
    ] {
        let active = candidate == state.filter;
        let _ = set_node_visible(popup, normal_id, !active);
        let _ = set_node_visible(popup, active_id, active);
        let hovered = state.hovered_filter == Some(candidate);
        set_runtime_filter_style(popup, normal_id, false, hovered);
        set_runtime_filter_style(popup, active_id, true, hovered);
    }
}

fn tooltip_at_point(
    root: &Node,
    assets: &Assets,
    x: f32,
    y: f32,
    dependency_tooltip: &str,
    dependency_warnings: &[(String, String)],
    pending_restart: bool,
) -> Option<String> {
    if node_contains_point(root, "owned_dependency_warning", x, y) && !dependency_tooltip.is_empty()
    {
        return Some(dependency_tooltip.to_owned());
    }
    if pending_restart && node_contains_point(root, "owned_restart_button", x, y) {
        return Some(
            "Close the game cleanly, relaunch it, and apply the pending mod changes.".to_owned(),
        );
    }
    let rows = find_node(root, "owned_rows")?;
    for row in &rows.child {
        if !row.visible || !node_contains_point(row, "owned_status_slot", x, y) {
            continue;
        }
        if find_visible_node(row, "owned_status_update", true).is_some() {
            return Some("An update is available for this mod.".to_owned());
        }
        if find_visible_node(row, "owned_status_warning", true).is_some() {
            let name = find_node(row, "owned_row_name")
                .and_then(|label| label_text(label, assets))
                .unwrap_or_default();
            return dependency_warnings
                .iter()
                .find(|(warning_name, _)| warning_name == &name)
                .map(|(_, tooltip)| tooltip.clone())
                .or_else(|| Some("This mod has a dependency warning.".to_owned()));
        }
        if find_visible_node(row, "owned_status_restart", true).is_some() {
            return Some("This mod change will be applied after restarting the game.".to_owned());
        }
    }
    None
}

fn sync_runtime_tooltip(root: &mut Node, text: Option<&str>) {
    let text = text.filter(|value| !value.trim().is_empty());
    let _ = set_node_visible(root, "owned_tooltip", text.is_some());
    let _ = set_node_visible(root, "owned_tooltip_text_overlay", text.is_some());
    if let Some(text) = text {
        let _ = set_label_text(root, "owned_tooltip_text_overlay", text);
    }
}

fn enabled_mod_count() -> Option<usize> {
    let game_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let source = fs::read_to_string(game_dir.join("config").join("game").join("mods.json")).ok()?;
    json_string_array_len(&source, "enabled_mods")
}

fn json_string_array_len(source: &str, key: &str) -> Option<usize> {
    let key = format!("\"{key}\"");
    let array = source
        .split_once(&key)?
        .1
        .split_once('[')?
        .1
        .split_once(']')?
        .0;
    let mut in_string = false;
    let mut escaped = false;
    let mut count = 0;
    for character in array.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_string => escaped = true,
            '"' => {
                in_string = !in_string;
                if in_string {
                    count += 1;
                }
            }
            _ => {}
        }
    }
    Some(count)
}

#[cfg(target_os = "windows")]
fn left_mouse_down() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(virtual_key: i32) -> i16;
    }
    game_window_has_focus() && (unsafe { GetAsyncKeyState(0x01) }) < 0
}

#[cfg(not(target_os = "windows"))]
fn left_mouse_down() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn current_key_mask() -> usize {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(virtual_key: i32) -> i16;
    }
    if !game_window_has_focus() {
        return 0;
    }
    let down = |key| unsafe { GetAsyncKeyState(key) } < 0;
    let mut mask = 0;
    for (key, bit) in [
        (KEY_UP, KEY_MASK_UP),
        (KEY_DOWN, KEY_MASK_DOWN),
        (KEY_HOME, KEY_MASK_HOME),
        (KEY_END, KEY_MASK_END),
        (KEY_E, KEY_MASK_ENABLE),
        (KEY_D, KEY_MASK_DISABLE),
    ] {
        if down(key) {
            mask |= bit;
        }
    }
    mask
}

#[cfg(not(target_os = "windows"))]
fn current_key_mask() -> usize {
    0
}

#[cfg(target_os = "windows")]
fn current_search_key_mask() -> usize {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(virtual_key: i32) -> i16;
    }
    if !game_window_has_focus() || (unsafe { GetAsyncKeyState(0x11) }) < 0 {
        return 0;
    }
    let down = |key| unsafe { GetAsyncKeyState(key) } < 0;
    let mut mask = 0;
    if down(0x08) {
        mask |= 1;
    }
    if down(0x20) {
        mask |= 2;
    }
    for index in 0..26 {
        if down(0x41 + index) {
            mask |= 1usize << (index + 2);
        }
    }
    for index in 0..10 {
        if down(0x30 + index) {
            mask |= 1usize << (index + 28);
        }
    }
    mask
}

#[cfg(not(target_os = "windows"))]
fn current_search_key_mask() -> usize {
    0
}

#[cfg(target_os = "windows")]
fn ctrl_f_down() -> bool {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetAsyncKeyState(virtual_key: i32) -> i16;
    }
    if !game_window_has_focus() {
        return false;
    }
    (unsafe { GetAsyncKeyState(0x11) }) < 0 && (unsafe { GetAsyncKeyState(0x46) }) < 0
}

#[cfg(not(target_os = "windows"))]
fn ctrl_f_down() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn cursor_ui_position(ui: &GameUI) -> Option<(f32, f32)> {
    use std::ffi::c_void;

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn ScreenToClient(window: *mut c_void, point: *mut Point) -> i32;
        fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
    }

    let window = unsafe { FindWindowW(std::ptr::null(), window_title().as_ptr()) };
    if window.is_null() || ui.rect.w <= 0.0 || ui.rect.h <= 0.0 {
        return None;
    }
    let mut point = Point { x: 0, y: 0 };
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetCursorPos(&mut point) } == 0
        || unsafe { ScreenToClient(window, &mut point) } == 0
        || unsafe { GetClientRect(window, &mut rect) } == 0
    {
        return None;
    }
    let width = (rect.right - rect.left) as f32;
    let height = (rect.bottom - rect.top) as f32;
    if width <= 0.0 || height <= 0.0 {
        return None;
    }
    Some((
        ui.rect.x + (point.x as f32 / width) * ui.rect.w,
        ui.rect.y + (point.y as f32 / height) * ui.rect.h,
    ))
}

#[cfg(not(target_os = "windows"))]
fn cursor_ui_position(_ui: &GameUI) -> Option<(f32, f32)> {
    None
}

#[cfg(target_os = "windows")]
fn request_clean_restart() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const WM_CLOSE: u32 = 0x0010;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut std::ffi::c_void;
        fn PostMessageW(
            window: *mut std::ffi::c_void,
            message: u32,
            wparam: usize,
            lparam: isize,
        ) -> i32;
    }

    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let Some(directory) = executable.parent() else {
        return false;
    };
    let escape = |value: &Path| value.to_string_lossy().replace('\'', "''");
    let script = format!(
        "Wait-Process -Id {} -ErrorAction SilentlyContinue; Start-Process -FilePath '{}' -WorkingDirectory '{}'",
        std::process::id(),
        escape(&executable),
        escape(directory)
    );
    if Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_err()
    {
        return false;
    }
    let window = unsafe { FindWindowW(std::ptr::null(), window_title().as_ptr()) };
    !window.is_null() && unsafe { PostMessageW(window, WM_CLOSE, 0, 0) } != 0
}

#[cfg(not(target_os = "windows"))]
fn request_clean_restart() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn post_ui_click(ui: &GameUI, ui_x: f32, ui_y: f32) -> bool {
    use std::ffi::c_void;

    #[repr(C)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
        fn GetClientRect(window: *mut c_void, rect: *mut Rect) -> i32;
        fn PostMessageW(window: *mut c_void, message: u32, wparam: usize, lparam: isize) -> i32;
    }

    const WM_MOUSEMOVE: u32 = 0x0200;
    const WM_LBUTTONDOWN: u32 = 0x0201;
    const WM_LBUTTONUP: u32 = 0x0202;
    const MK_LBUTTON: usize = 0x0001;

    let window = unsafe { FindWindowW(std::ptr::null(), window_title().as_ptr()) };
    if window.is_null() || ui.rect.w <= 0.0 || ui.rect.h <= 0.0 {
        return false;
    }
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if unsafe { GetClientRect(window, &mut rect) } == 0 {
        return false;
    }
    let width = (rect.right - rect.left) as f32;
    let height = (rect.bottom - rect.top) as f32;
    if width <= 0.0 || height <= 0.0 {
        return false;
    }
    let x = (((ui_x - ui.rect.x) / ui.rect.w) * width)
        .round()
        .clamp(0.0, width - 1.0) as i32;
    let y = (((ui_y - ui.rect.y) / ui.rect.h) * height)
        .round()
        .clamp(0.0, height - 1.0) as i32;
    let position = ((y as isize) << 16) | (x as isize & 0xffff);

    let moved = unsafe { PostMessageW(window, WM_MOUSEMOVE, 0, position) } != 0;
    let pressed = unsafe { PostMessageW(window, WM_LBUTTONDOWN, MK_LBUTTON, position) } != 0;
    let released = unsafe { PostMessageW(window, WM_LBUTTONUP, 0, position) } != 0;
    moved && pressed && released
}

#[cfg(not(target_os = "windows"))]
fn post_ui_click(_ui: &GameUI, _ui_x: f32, _ui_y: f32) -> bool {
    false
}

fn window_title() -> &'static [u16] {
    &[
        84, 101, 97, 109, 102, 105, 103, 104, 116, 32, 77, 97, 110, 97, 103, 101, 114, 50, 0,
    ]
}

#[cfg(target_os = "windows")]
fn game_window_has_focus() -> bool {
    use std::ffi::c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
        fn GetForegroundWindow() -> *mut c_void;
    }

    let game = unsafe { FindWindowW(std::ptr::null(), window_title().as_ptr()) };
    !game.is_null() && unsafe { GetForegroundWindow() } == game
}

#[cfg(not(target_os = "windows"))]
fn game_window_has_focus() -> bool {
    false
}

fn init(_ctx: &GameCtx) -> ModRegistration {
    let mut registration = ModRegistration::new(MOD_ID);
    registration.set_extension(BetterModMenuExtension {
        popup_open: AtomicBool::new(false),
        pending_selection: AtomicUsize::new(0),
        selected_index: AtomicUsize::new(0),
        key_state: AtomicUsize::new(0),
        text_key_state: AtomicUsize::new(0),
        shortcut_down: AtomicBool::new(false),
        restarting: AtomicBool::new(false),
        mouse_down: AtomicBool::new(false),
        toggle_reselect_pending: AtomicBool::new(false),
        poll_tick: AtomicUsize::new(0),
        styled_row_count: AtomicUsize::new(0),
        enabled_count: AtomicUsize::new(0),
        search_focused: AtomicBool::new(false),
        filter_hovered: AtomicBool::new(false),
        filter: AtomicUsize::new(FILTER_ALL),
        query: Mutex::new(String::new()),
        selected_name: Mutex::new(String::new()),
        baseline_mod_states: Mutex::new(Vec::new()),
        toggle_transitions: Mutex::new(Vec::new()),
        settings_catalog: Mutex::new(ModSettingsCatalog::default()),
    });
    registration
}

declare_mod!(init);

#[cfg(test)]
mod tests {
    use super::{compare_versions, is_safe_mod_id, json_string_array_len, version_satisfies};
    use std::cmp::Ordering;

    #[test]
    fn rejects_mod_ids_that_can_escape_the_storage_root() {
        assert!(is_safe_mod_id("intro_skip"));
        assert!(is_safe_mod_id("3773405658"));
        assert!(!is_safe_mod_id(""));
        assert!(!is_safe_mod_id("../intro_skip"));
        assert!(!is_safe_mod_id("folder/intro_skip"));
        assert!(!is_safe_mod_id(r"C:\intro_skip"));
    }

    #[test]
    fn evaluates_supported_version_ranges() {
        assert!(version_satisfies("0.5.3", ">=0.5.2, <0.5.4"));
        assert!(!version_satisfies("0.5.4", ">=0.5.2, <0.5.4"));
        assert_eq!(compare_versions("v0.5.3", "0.5.3.0"), Ordering::Equal);
    }

    #[test]
    fn counts_only_json_array_strings() {
        let source = r#"{"enabled_mods":["intro_skip","mod_menu"],"other":1}"#;
        assert_eq!(json_string_array_len(source, "enabled_mods"), Some(2));
    }
}
