// SPDX-License-Identifier: GPL-3.0-or-later
// See LICENSE-EXCEPTION.md for the TFM2 linking exception and attribution terms.

use mod_api_stable::*;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    Arc, Mutex,
};

mod dependencies;
mod integration;
mod io;
mod model;

const MOD_ID: &str = "tfm2_better_mod_menu";
const MOD_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_REVISION: &str = match option_env!("TFM2_BMM_BUILD_REVISION") {
    Some(revision) => revision,
    None => "development",
};
const BUILD_TIMESTAMP: &str = match option_env!("TFM2_BMM_BUILD_TIMESTAMP") {
    Some(timestamp) => timestamp,
    None => "unknown",
};
const RUNTIME_LAYOUT_ASSET: &str = "asset/tfm2_better_mod_menu/ui/layout/better_mod_menu_runtime";
const ROOT: &str = "body.mods_popup.bmm_surface";
const NATIVE_LEFT: &str = "body.mods_popup.left_panel";
const NATIVE_RIGHT: &str = "body.mods_popup.right_panel";
const MAX_VISIBLE_ROWS: usize = 10;
const KEY_UP: i32 = 0x26;
const KEY_DOWN: i32 = 0x28;
const KEY_HOME: i32 = 0x24;
const KEY_END: i32 = 0x23;
const KEY_E: i32 = 0x45;
const KEY_D: i32 = 0x44;
const KEY_ESCAPE: i32 = 0x1b;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
enum SourceFilter {
    #[default]
    All = 0,
    Local = 1,
    Workshop = 2,
}

impl SourceFilter {
    fn from_raw(raw: u8) -> Self {
        match raw {
            1 => Self::Local,
            2 => Self::Workshop,
            _ => Self::All,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ModInfo {
    mod_id: String,
    name: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug, Deserialize)]
struct Dependency {
    mod_id: String,
    version: String,
}

#[derive(Clone, Debug)]
struct ModEntry {
    info: ModInfo,
    root: PathBuf,
    workshop: bool,
    enabled: bool,
}

impl ModEntry {
    fn search_text(&self) -> String {
        format!("{} {}", self.info.name, self.info.author).to_lowercase()
    }

    fn matches(&self, query: &str, filter: SourceFilter) -> bool {
        let source_matches = match filter {
            SourceFilter::All => true,
            SourceFilter::Local => !self.workshop,
            SourceFilter::Workshop => self.workshop,
        };
        source_matches && self.search_text().contains(&query.to_lowercase())
    }
}

struct SharedState {
    mods: Mutex<Vec<ModEntry>>,
    query: Mutex<String>,
    filter: AtomicU8,
    selected: AtomicUsize,
    page_start: AtomicUsize,
    dirty: AtomicBool,
    restart_required: AtomicBool,
    settings_view: AtomicBool,
}

impl SharedState {
    fn new(mods: Vec<ModEntry>) -> Self {
        let selected = mods
            .iter()
            .position(|entry| entry.info.mod_id == "tfm2_intro_skip")
            .or_else(|| mods.iter().position(|entry| entry.info.mod_id == MOD_ID))
            .unwrap_or(0);
        Self {
            mods: Mutex::new(mods),
            query: Mutex::new(String::new()),
            filter: AtomicU8::new(SourceFilter::All as u8),
            selected: AtomicUsize::new(selected),
            page_start: AtomicUsize::new(0),
            dirty: AtomicBool::new(true),
            restart_required: AtomicBool::new(false),
            settings_view: AtomicBool::new(false),
        }
    }
}

struct BetterModMenu {
    shared: Arc<SharedState>,
    handlers_registered: AtomicBool,
    last_search: Mutex<String>,
    last_selected: AtomicUsize,
    last_key_mask: AtomicUsize,
}

impl StableExtension for BetterModMenu {
    fn post_update(&self, ctx: &mut StableClient<'_>, _dt_micros: u64) {
        if ctx.scene_kind() != Some(SceneKindV1::Title) || !ctx.ui_exists("body.mods_popup") {
            self.handlers_registered.store(false, Ordering::Release);
            return;
        }

        if !ctx.ui_exists(ROOT) {
            if !ctx.ui_spawn_template("body.mods_popup", RUNTIME_LAYOUT_ASSET, true) {
                return;
            }
            self.shared.dirty.store(true, Ordering::Release);
            self.spawn_rows(ctx);
            self.spawn_settings_rows(ctx);
        }

        ctx.ui_set_visible(NATIVE_LEFT, false);
        ctx.ui_set_visible(NATIVE_RIGHT, false);

        if !self.handlers_registered.swap(true, Ordering::AcqRel) {
            self.register_handlers(ctx);
        }

        let search_path = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_search_input");
        let search = ctx.ui_text_edit_text(&search_path).unwrap_or_default();
        if let Ok(mut previous) = self.last_search.lock() {
            if *previous != search {
                *previous = search.clone();
                if let Ok(mut query) = self.shared.query.lock() {
                    *query = search.clone();
                }
                self.shared.page_start.store(0, Ordering::Release);
                self.shared.dirty.store(true, Ordering::Release);
            }
        }

        let selected = self.shared.selected.load(Ordering::Acquire);
        if self.last_selected.swap(selected, Ordering::AcqRel) != selected {
            self.shared.dirty.store(true, Ordering::Release);
        }

        self.handle_keyboard(ctx);

        if self.shared.dirty.swap(false, Ordering::AcqRel) {
            self.render(ctx, &search);
        }

        let restart_path = format!("{ROOT}.bmm_footer.bmm_footer_right.bmm_restart_apply_button");
        ctx.ui_set_visible(
            &restart_path,
            self.shared.restart_required.load(Ordering::Acquire),
        );
    }
}

impl BetterModMenu {
    fn handle_keyboard(&self, ctx: &mut StableClient<'_>) {
        let mask = current_key_mask();
        let pressed = mask & !self.last_key_mask.swap(mask, Ordering::AcqRel);
        if pressed == 0 {
            return;
        }
        if pressed & (1 << 6) != 0 {
            let _ = ctx.ui_set_visible("body.mods_popup", false);
            return;
        }

        let visible = filtered_indices(&self.shared);
        if visible.is_empty() {
            return;
        }
        let current = self.shared.selected.load(Ordering::Acquire);
        let position = visible
            .iter()
            .position(|index| *index == current)
            .unwrap_or(0);
        let next = if pressed & (1 << 0) != 0 {
            position.saturating_sub(1)
        } else if pressed & (1 << 1) != 0 {
            (position + 1).min(visible.len() - 1)
        } else if pressed & (1 << 2) != 0 {
            0
        } else if pressed & (1 << 3) != 0 {
            visible.len() - 1
        } else {
            position
        };
        if next != position {
            self.shared.selected.store(visible[next], Ordering::Release);
            let start = self.shared.page_start.load(Ordering::Acquire);
            if next < start {
                self.shared.page_start.store(next, Ordering::Release);
            } else if next >= start + MAX_VISIBLE_ROWS {
                self.shared
                    .page_start
                    .store(next + 1 - MAX_VISIBLE_ROWS, Ordering::Release);
            }
            self.shared.dirty.store(true, Ordering::Release);
        }
        if pressed & (1 << 4) != 0 {
            set_entry_enabled(&self.shared, visible[next], true);
        } else if pressed & (1 << 5) != 0 {
            set_entry_enabled(&self.shared, visible[next], false);
        }
    }

    fn spawn_rows(&self, ctx: &mut StableClient<'_>) {
        let parent = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_rows");
        for slot in 0..MAX_VISIBLE_ROWS {
            let _ = ctx.ui_spawn_source(&parent, &row_source(slot));
        }
    }

    fn register_handlers(&self, ctx: &mut StableClient<'_>) {
        let shared = Arc::clone(&self.shared);
        let path =
            format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_filter_all.native_filter_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| {
            shared
                .filter
                .store(SourceFilter::All as u8, Ordering::Release);
            shared.page_start.store(0, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        let shared = Arc::clone(&self.shared);
        let path =
            format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_filter_code.native_filter_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| {
            shared
                .filter
                .store(SourceFilter::Local as u8, Ordering::Release);
            shared.page_start.store(0, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        let shared = Arc::clone(&self.shared);
        let path = format!(
            "{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_filter_workshop.native_filter_hitbox"
        );
        let _ = ctx.ui_register_click(&path, "", move |_| {
            shared
                .filter
                .store(SourceFilter::Workshop as u8, Ordering::Release);
            shared.page_start.store(0, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        let shared = Arc::clone(&self.shared);
        let path = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_table_header.bmm_enable_all.native_filter_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| set_all_enabled(&shared, true));

        let shared = Arc::clone(&self.shared);
        let path = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_table_header.bmm_disable_all.native_filter_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| set_all_enabled(&shared, false));

        let shared = Arc::clone(&self.shared);
        let clear = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_search_clear");
        let search = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_search_input");
        let _ = ctx.ui_register_click(&clear, "", move |ctx| {
            let _ = ctx.ui_set_text_edit_text(&search, "");
            shared.page_start.store(0, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        let close =
            format!("{ROOT}.bmm_footer.bmm_footer_right.bmm_close_menu_button.close_hitbox");
        let _ = ctx.ui_register_click(&close, "", move |ctx| {
            let _ = ctx.ui_set_visible("body.mods_popup", false);
        });

        let workshop = format!(
            "{ROOT}.bmm_footer.bmm_footer_right.bmm_open_workshop_button.open_workshop_hitbox"
        );
        let _ = ctx.ui_register_click(&workshop, "", move |_| {
            let _ = open_https_url("https://steamcommunity.com/app/3009300/workshop/");
        });

        let restart = format!(
            "{ROOT}.bmm_footer.bmm_footer_right.bmm_restart_apply_button.native_restart_hitbox"
        );
        let _ = ctx.ui_register_click(&restart, "", move |_| {
            let _ = request_clean_restart();
        });

        let shared = Arc::clone(&self.shared);
        let path = format!("{ROOT}.bmm_menu_grid.bmm_details_panel.bmm_mod_content_card.bmm_mod_tab_overview.native_overview_tab_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| {
            shared.settings_view.store(false, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        let shared = Arc::clone(&self.shared);
        let path = format!("{ROOT}.bmm_menu_grid.bmm_details_panel.bmm_mod_content_card.bmm_mod_tab_settings.native_settings_tab_hitbox");
        let _ = ctx.ui_register_click(&path, "", move |_| {
            shared.settings_view.store(true, Ordering::Release);
            shared.dirty.store(true, Ordering::Release);
        });

        for (row, key) in [
            ("stable_setting_skip", "skip_disclaimer"),
            ("stable_setting_continue", "auto_continue"),
            ("stable_setting_mismatch", "auto_load_anyway"),
        ] {
            let shared = Arc::clone(&self.shared);
            let path = setting_path(row, "on_hitbox");
            let _ = ctx.ui_register_click(&path, "", move |_| {
                let _ = write_intro_setting(key, Value::Bool(true));
                shared.dirty.store(true, Ordering::Release);
            });
            let shared = Arc::clone(&self.shared);
            let path = setting_path(row, "off_hitbox");
            let _ = ctx.ui_register_click(&path, "", move |_| {
                let _ = write_intro_setting(key, Value::Bool(false));
                shared.dirty.store(true, Ordering::Release);
            });
        }

        for (child, direction) in [("previous", -1_i32), ("next", 1_i32)] {
            let shared = Arc::clone(&self.shared);
            let path = setting_path("stable_setting_retention", child);
            let _ = ctx.ui_register_click(&path, "", move |_| {
                let _ = cycle_retention(direction);
                shared.dirty.store(true, Ordering::Release);
            });
        }

        for slot in 0..MAX_VISIBLE_ROWS {
            let shared = Arc::clone(&self.shared);
            let path = row_path(slot, "select_hitbox");
            let _ = ctx.ui_register_click(&path, "", move |_| select_visible_slot(&shared, slot));

            let shared = Arc::clone(&self.shared);
            let path = row_path(slot, "enable_hitbox");
            let _ = ctx.ui_register_click(&path, "", move |_| {
                set_visible_slot_enabled(&shared, slot, true)
            });

            let shared = Arc::clone(&self.shared);
            let path = row_path(slot, "disable_hitbox");
            let _ = ctx.ui_register_click(&path, "", move |_| {
                set_visible_slot_enabled(&shared, slot, false)
            });
        }
    }

    fn render(&self, ctx: &mut StableClient<'_>, search: &str) {
        let Ok(mods) = self.shared.mods.lock() else {
            return;
        };
        let filter = SourceFilter::from_raw(self.shared.filter.load(Ordering::Acquire));
        let visible: Vec<usize> = mods
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.matches(search, filter).then_some(index))
            .collect();
        let start = self
            .shared
            .page_start
            .load(Ordering::Acquire)
            .min(visible.len().saturating_sub(1));
        let enabled = mods.iter().filter(|entry| entry.enabled).count();
        let summary = format!(
            "{} installed | {} enabled | {} disabled",
            mods.len(),
            enabled,
            mods.len().saturating_sub(enabled)
        );
        let _ = ctx.ui_set_text(
            &format!("{ROOT}.bmm_header.bmm_mod_count_summary"),
            &summary,
        );

        let clear_path = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_search_clear");
        let _ = ctx.ui_set_visible(&clear_path, !search.is_empty());
        self.render_filter_state(ctx, filter);

        for slot in 0..MAX_VISIBLE_ROWS {
            let Some(index) = visible.get(start + slot).copied() else {
                let _ = ctx.ui_set_visible(&row_path(slot, ""), false);
                continue;
            };
            let entry = &mods[index];
            let root = row_path(slot, "");
            let _ = ctx.ui_set_visible(&root, true);
            let _ = ctx.ui_set_text(&row_path(slot, "name"), &entry.info.name);
            let _ = ctx.ui_set_text(&row_path(slot, "author"), &entry.info.author);
            let _ = ctx.ui_set_text(
                &row_path(slot, "source"),
                if entry.workshop { "Workshop" } else { "Local" },
            );
            let _ = ctx.ui_set_visible(&row_path(slot, "enabled_mark"), true);
            let _ = ctx.ui_set_visible(&row_path(slot, "disabled_mark"), true);
            let _ = ctx.ui_set_properties(
                &row_path(slot, "enabled_mark"),
                if entry.enabled {
                    "color: #145648ff; back_color: #145648ff; stroke: 1;"
                } else {
                    "color: #1d1f2cff; back_color: #1d1f2cff; stroke: 1;"
                },
            );
            let _ = ctx.ui_set_properties(
                &row_path(slot, "disabled_mark"),
                if entry.enabled {
                    "color: #1d1f2cff; back_color: #1d1f2cff; stroke: 1;"
                } else {
                    "color: #4a171cff; back_color: #4a171cff; stroke: 1;"
                },
            );
            let selected = self.shared.selected.load(Ordering::Acquire) == index;
            let _ = ctx.ui_set_properties(
                &root,
                if selected {
                    "stroke: 1; back_color: #202331ff; color: #202331ff;"
                } else {
                    "stroke: 0; back_color: #1d1f2cff; color: #1d1f2cff;"
                },
            );
        }

        let empty = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_list_empty");
        let _ = ctx.ui_set_visible(&empty, visible.is_empty());

        let selected = self
            .shared
            .selected
            .load(Ordering::Acquire)
            .min(mods.len().saturating_sub(1));
        if let Some(entry) = mods.get(selected) {
            self.render_details(ctx, entry);
        }
    }

    fn render_filter_state(&self, ctx: &mut StableClient<'_>, filter: SourceFilter) {
        let base = format!("{ROOT}.bmm_menu_grid.bmm_library_panel");
        for (name, value) in [
            ("all", SourceFilter::All),
            ("code", SourceFilter::Local),
            ("workshop", SourceFilter::Workshop),
        ] {
            let _ = ctx.ui_set_visible(&format!("{base}.bmm_filter_{name}"), filter != value);
            let _ =
                ctx.ui_set_visible(&format!("{base}.bmm_filter_{name}_active"), filter == value);
        }
    }

    fn render_details(&self, ctx: &mut StableClient<'_>, entry: &ModEntry) {
        let base = format!("{ROOT}.bmm_menu_grid.bmm_details_panel");
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_info_card.bmm_mod_name"),
            &entry.info.name,
        );
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_info_card.bmm_version"),
            &format!("Version {}", entry.info.version),
        );
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_info_card.bmm_mod_author"),
            &entry.info.author,
        );
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_info_card.bmm_mod_source"),
            if entry.workshop { "Workshop" } else { "Local" },
        );
        let dependencies = if entry.info.dependencies.is_empty() {
            "No declared dependencies".to_owned()
        } else {
            entry
                .info
                .dependencies
                .iter()
                .map(|dependency| {
                    if dependency.mod_id == "base" {
                        format!("TFM2 version {}", dependency.version)
                    } else {
                        format!("{} {}", dependency.mod_id, dependency.version)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_info_card.bmm_mod_dependencies"),
            &dependencies,
        );
        let _ = ctx.ui_set_text(
            &format!("{base}.bmm_mod_content_card.bmm_mod_description_scroll.contents.bmm_mod_description"),
            &entry.info.description,
        );
        let _ = ctx.ui_set_properties(
            &format!("{base}.bmm_mod_thumbnail_card.bmm_mod_thumbnail"),
            &format!("source: \"asset/{}/thumbnail\";", entry.info.mod_id),
        );
        let has_thumbnail = entry.root.join("thumbnail.png").is_file()
            || entry.root.join("thumbnail.jpg").is_file();
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_thumbnail_card.bmm_mod_thumbnail"),
            has_thumbnail,
        );
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_thumbnail_card.bmm_mod_thumbnail_fallback"),
            !has_thumbnail,
        );
        let _ = ctx.ui_set_properties(
            &format!(
                "{base}.bmm_mod_content_card.bmm_mod_description_scroll.contents.bmm_mod_banner"
            ),
            &format!("source: \"asset/{}/banner\";", entry.info.mod_id),
        );
        let has_banner =
            entry.root.join("banner.png").is_file() || entry.root.join("banner.jpg").is_file();
        let _ = ctx.ui_set_visible(
            &format!(
                "{base}.bmm_mod_content_card.bmm_mod_description_scroll.contents.bmm_mod_banner"
            ),
            has_banner,
        );
        let has_settings = entry.info.mod_id == "tfm2_intro_skip"
            && entry.root.join("better_mod_menu.json").is_file();
        let settings_view = has_settings && self.shared.settings_view.load(Ordering::Acquire);
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_content_card.bmm_mod_tab_overview"),
            has_settings,
        );
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_content_card.bmm_mod_tab_settings"),
            has_settings,
        );
        let description = format!("{base}.bmm_mod_content_card.bmm_mod_description_scroll");
        let settings_panel = format!("{base}.bmm_mod_content_card.bmm_settings_panel");
        let _ = ctx.ui_set_visible(&description, !settings_view);
        let _ = ctx.ui_set_visible(&settings_panel, settings_view);
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_content_card.bmm_mod_overview_icon"),
            !settings_view,
        );
        let _ = ctx.ui_set_visible(
            &format!("{base}.bmm_mod_content_card.bmm_mod_overview_title"),
            !settings_view,
        );
        let _ = ctx.ui_set_properties(
            &format!("{base}.bmm_mod_content_card.bmm_mod_tab_overview"),
            if settings_view {
                "back_color: #252735ff; color: #252735ff;"
            } else {
                "back_color: #145648ff; color: #145648ff;"
            },
        );
        let _ = ctx.ui_set_properties(
            &format!("{base}.bmm_mod_content_card.bmm_mod_tab_settings"),
            if settings_view {
                "back_color: #145648ff; color: #145648ff;"
            } else {
                "back_color: #252735ff; color: #252735ff;"
            },
        );
        if settings_view {
            render_intro_settings(ctx);
        }
    }

    fn spawn_settings_rows(&self, ctx: &mut StableClient<'_>) {
        let parent = format!("{ROOT}.bmm_menu_grid.bmm_details_panel.bmm_mod_content_card.bmm_settings_panel.bmm_settings_rows");
        for (id, label, description) in [
            (
                "stable_setting_skip",
                "Skip disclaimer",
                "Dismiss the startup disclaimer automatically.",
            ),
            (
                "stable_setting_continue",
                "Continue latest career",
                "Press Continue automatically when the title screen is ready.",
            ),
            (
                "stable_setting_mismatch",
                "Load through mod mismatch",
                "Continue only after Intro Skip verifies a fresh backup.",
            ),
        ] {
            let _ = ctx.ui_spawn_source(&parent, &toggle_setting_source(id, label, description));
        }
        let _ = ctx.ui_spawn_source(&parent, &choice_setting_source());
    }
}

fn setting_path(row: &str, child: &str) -> String {
    format!("{ROOT}.bmm_menu_grid.bmm_details_panel.bmm_mod_content_card.bmm_settings_panel.bmm_settings_rows.{row}.{child}")
}

fn toggle_setting_source(id: &str, label: &str, description: &str) -> String {
    format!(
        r#"{id}:color {{
  width: 694px; height: 76px; color: #202230ff; back_color: #202230ff; stroke: 1;
  #title:label {{ @"asset/base/style/main#label"; x: 14px; y: 8px; width: 470px; height: 24px; size: 14; font: "asset/base/font/set/regular"; text: "{label}"; }}
  #description:label {{ @"asset/base/style/main#label"; x: 14px; y: 34px; width: 470px; height: 24px; size: 12; font: "asset/base/font/set/regular"; text: "{description}"; }}
  #on:color {{ x: 510px; y: 20px; width: 78px; height: 36px; color: #1d1f2cff; back_color: #1d1f2cff; stroke: 1; #label:label {{ @"asset/base/style/main#bold_label"; width: 78px; height: 36px; size: 12; color: #f2f2f4ff; align_x: Center; align_y: Center; text: "ON"; }} }}
  #off:color {{ x: 590px; y: 20px; width: 78px; height: 36px; color: #1d1f2cff; back_color: #1d1f2cff; stroke: 1; #label:label {{ @"asset/base/style/main#bold_label"; width: 78px; height: 36px; size: 12; color: #f2f2f4ff; align_x: Center; align_y: Center; text: "OFF"; }} }}
  #on_hitbox:button {{ x: 510px; y: 20px; width: 78px; height: 36px; source: ""; color: #00000000; hover: {{ color: #00000000; }} active: {{ color: #00000000; }} }}
  #off_hitbox:button {{ x: 590px; y: 20px; width: 78px; height: 36px; source: ""; color: #00000000; hover: {{ color: #00000000; }} active: {{ color: #00000000; }} }}
}}"#
    )
}

fn choice_setting_source() -> String {
    r#"stable_setting_retention:color {
  width: 694px; height: 76px; color: #202230ff; back_color: #202230ff; stroke: 1;
  #title:label { @"asset/base/style/main#label"; x: 14px; y: 8px; width: 470px; height: 24px; size: 14; font: "asset/base/font/set/regular"; text: "Backup retention"; }
  #description:label { @"asset/base/style/main#label"; x: 14px; y: 34px; width: 470px; height: 24px; size: 12; font: "asset/base/font/set/regular"; text: "How long managed safety backups are retained."; }
  #value:label { @"asset/base/style/main#label"; x: 548px; y: 20px; width: 82px; height: 36px; size: 12; align_x: Center; align_y: Center; text: "7 days"; }
  #previous:button { x: 510px; y: 20px; width: 36px; height: 36px; source: "asset/base/ui/icons/left_arrow"; }
  #next:button { x: 632px; y: 20px; width: 36px; height: 36px; source: "asset/base/ui/icons/right_arrow"; }
}"#.to_owned()
}

fn row_source(slot: usize) -> String {
    format!(
        r#"row_{slot}:color {{
  width: 734px; height: 52px; color: #1d1f2cff; back_color: #1d1f2cff;
  #name:label {{ @"asset/base/style/main#label"; x: 12px; width: 244px; height: 52px; size: 15; font: "asset/base/font/set/regular"; color: #f2f2f4ff; fit_width: true; align_x: Left; align_y: Center; text: ""; }}
  #author:label {{ @"asset/base/style/main#label"; x: 268px; width: 154px; height: 52px; size: 14; font: "asset/base/font/set/regular"; color: #f2f2f4ff; fit_width: true; align_x: Left; align_y: Center; text: ""; }}
  #source:label {{ @"asset/base/style/main#label"; x: 422px; width: 80px; height: 52px; size: 12; font: "asset/base/font/set/regular"; color: #f2f2f4ff; align_x: Center; align_y: Center; text: ""; }}
  #enabled_mark:color {{ x: 508px; y: 8px; width: 82px; height: 36px; color: #145648ff; back_color: #145648ff; #text:label {{ @"asset/base/style/main#bold_label"; width: 82px; height: 36px; size: 13; color: #f2f2f4ff; align_x: Center; align_y: Center; text: "Enabled"; }} }}
  #disabled_mark:color {{ x: 594px; y: 8px; width: 82px; height: 36px; color: #4a171cff; back_color: #4a171cff; #text:label {{ @"asset/base/style/main#bold_label"; width: 82px; height: 36px; size: 13; color: #f2f2f4ff; align_x: Center; align_y: Center; text: "Disabled"; }} }}
  #select_hitbox:button {{ width: 508px; height: 52px; source: ""; color: #00000000; hover: {{ color: #00000000; }} active: {{ color: #00000000; }} }}
  #enable_hitbox:button {{ x: 508px; y: 8px; width: 82px; height: 36px; source: ""; color: #00000000; hover: {{ color: #00000000; }} active: {{ color: #00000000; }} }}
  #disable_hitbox:button {{ x: 594px; y: 8px; width: 82px; height: 36px; source: ""; color: #00000000; hover: {{ color: #00000000; }} active: {{ color: #00000000; }} }}
}}"#
    )
}

fn row_path(slot: usize, child: &str) -> String {
    let base = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_rows.row_{slot}");
    if child.is_empty() {
        base
    } else {
        format!("{base}.{child}")
    }
}

fn visible_indices_for(mods: &[ModEntry], query: &str, filter: SourceFilter) -> Vec<usize> {
    mods.iter()
        .enumerate()
        .filter_map(|(index, entry)| entry.matches(query, filter).then_some(index))
        .collect()
}

fn filtered_indices(shared: &SharedState) -> Vec<usize> {
    let Ok(mods) = shared.mods.lock() else {
        return Vec::new();
    };
    let query = shared
        .query
        .lock()
        .map(|query| query.clone())
        .unwrap_or_default();
    let filter = SourceFilter::from_raw(shared.filter.load(Ordering::Acquire));
    visible_indices_for(&mods, &query, filter)
}

fn select_visible_slot(shared: &SharedState, slot: usize) {
    let visible = filtered_indices(shared);
    let start = shared.page_start.load(Ordering::Acquire);
    if let Some(index) = visible.get(start + slot) {
        shared.selected.store(*index, Ordering::Release);
        shared.dirty.store(true, Ordering::Release);
    }
}

fn set_visible_slot_enabled(shared: &SharedState, slot: usize, enabled: bool) {
    let visible = filtered_indices(shared);
    let start = shared.page_start.load(Ordering::Acquire);
    if let Some(index) = visible.get(start + slot).copied() {
        set_entry_enabled(shared, index, enabled);
    }
}

fn set_all_enabled(shared: &SharedState, enabled: bool) {
    let ids = if let Ok(mods) = shared.mods.lock() {
        mods.iter()
            .map(|entry| entry.info.mod_id.clone())
            .collect::<Vec<_>>()
    } else {
        return;
    };
    if update_enabled_mods(&ids, enabled).is_ok() {
        if let Ok(mut mods) = shared.mods.lock() {
            for entry in &mut *mods {
                entry.enabled = enabled;
            }
        }
        shared.restart_required.store(true, Ordering::Release);
        shared.dirty.store(true, Ordering::Release);
    }
}

fn set_entry_enabled(shared: &SharedState, index: usize, enabled: bool) {
    let id = if let Ok(mods) = shared.mods.lock() {
        match mods.get(index) {
            Some(entry) if entry.enabled != enabled => entry.info.mod_id.clone(),
            _ => return,
        }
    } else {
        return;
    };
    if update_enabled_mods(std::slice::from_ref(&id), enabled).is_ok() {
        if let Ok(mut mods) = shared.mods.lock() {
            if let Some(entry) = mods.get_mut(index) {
                entry.enabled = enabled;
            }
        }
        shared.restart_required.store(true, Ordering::Release);
        shared.dirty.store(true, Ordering::Release);
    }
}

fn update_enabled_mods(ids: &[String], enabled: bool) -> Result<(), String> {
    let path = game_root().join("config/game/mods.json");
    let source = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let mut document: Value = serde_json::from_str(&source).map_err(|error| error.to_string())?;
    set_enabled_in_document(&mut document, ids, enabled)?;
    let temp = path.with_extension("json.bmm.tmp");
    fs::write(
        &temp,
        serde_json::to_vec(&document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(&temp, &path).map_err(|error| error.to_string())
}

fn set_enabled_in_document(
    document: &mut Value,
    ids: &[String],
    enabled: bool,
) -> Result<(), String> {
    let array = document
        .get_mut("enabled_mods")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "enabled_mods is missing".to_owned())?;
    let targets: HashSet<&str> = ids.iter().map(String::as_str).collect();
    array.retain(|value| value.as_str().is_none_or(|id| !targets.contains(id)));
    if enabled {
        array.extend(ids.iter().map(|id| json!(id)));
    }
    Ok(())
}

fn game_root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn scan_installed_mods() -> Vec<ModEntry> {
    let root = game_root();
    let enabled = enabled_mod_ids(&root);
    let mut roots = vec![(root.join("mods"), false)];
    if let Some(steamapps) = root.parent().and_then(Path::parent) {
        roots.push((steamapps.join("workshop/content/3009300"), true));
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for (scan_root, workshop) in roots {
        for candidate in mod_info_candidates(&scan_root) {
            let Ok(source) = fs::read_to_string(candidate.join("mod.mod_info")) else {
                continue;
            };
            let Ok(info) = serde_json::from_str::<ModInfo>(&source) else {
                continue;
            };
            if info.mod_id == "base" || !seen.insert(info.mod_id.clone()) {
                continue;
            }
            result.push(ModEntry {
                enabled: enabled.contains(&info.mod_id),
                info,
                root: candidate,
                workshop,
            });
        }
    }
    result.sort_by(|left, right| {
        left.info
            .name
            .to_lowercase()
            .cmp(&right.info.name.to_lowercase())
    });
    result
}

fn mod_info_candidates(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.join("mod.mod_info").is_file() {
            result.push(path.clone());
        }
        if let Ok(children) = fs::read_dir(&path) {
            for child in children.flatten() {
                let child = child.path();
                if child.join("mod.mod_info").is_file() {
                    result.push(child);
                }
            }
        }
    }
    result
}

fn enabled_mod_ids(root: &Path) -> HashSet<String> {
    let Ok(source) = fs::read_to_string(root.join("config/game/mods.json")) else {
        return HashSet::new();
    };
    let Ok(document) = serde_json::from_str::<Value>(&source) else {
        return HashSet::new();
    };
    document
        .get("enabled_mods")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn intro_settings_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("TeamSamoyed/TeamfightManager2/data/tfm2_intro_skip/settings.json"),
    )
}

fn read_intro_settings() -> Value {
    intro_settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|source| serde_json::from_str(&source).ok())
        .unwrap_or_else(|| {
            json!({
                "skip_disclaimer": true,
                "auto_continue": false,
                "auto_load_anyway": false,
                "backup_retention_days": 7
            })
        })
}

fn write_intro_setting(key: &str, value: Value) -> Result<(), String> {
    let path = intro_settings_path().ok_or_else(|| "APPDATA is unavailable".to_owned())?;
    let mut document = read_intro_settings();
    let object = document
        .as_object_mut()
        .ok_or_else(|| "Intro Skip settings must be a JSON object".to_owned())?;
    object.insert(key.to_owned(), value);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temp = path.with_extension("json.bmm.tmp");
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    fs::rename(temp, path).map_err(|error| error.to_string())
}

fn cycle_retention(direction: i32) -> Result<(), String> {
    const OPTIONS: [i64; 4] = [3, 7, 14, 30];
    let current = read_intro_settings()
        .get("backup_retention_days")
        .and_then(Value::as_i64)
        .unwrap_or(7);
    let index = OPTIONS
        .iter()
        .position(|value| *value == current)
        .unwrap_or(1) as i32;
    let next = (index + direction).rem_euclid(OPTIONS.len() as i32) as usize;
    write_intro_setting("backup_retention_days", json!(OPTIONS[next]))
}

fn render_intro_settings(ctx: &mut StableClient<'_>) {
    let document = read_intro_settings();
    for (row, key) in [
        ("stable_setting_skip", "skip_disclaimer"),
        ("stable_setting_continue", "auto_continue"),
        ("stable_setting_mismatch", "auto_load_anyway"),
    ] {
        let enabled = document.get(key).and_then(Value::as_bool).unwrap_or(false);
        let _ = ctx.ui_set_properties(
            &setting_path(row, "on"),
            if enabled {
                "color: #145648ff; back_color: #145648ff;"
            } else {
                "color: #1d1f2cff; back_color: #1d1f2cff;"
            },
        );
        let _ = ctx.ui_set_properties(
            &setting_path(row, "off"),
            if enabled {
                "color: #1d1f2cff; back_color: #1d1f2cff;"
            } else {
                "color: #4a171cff; back_color: #4a171cff;"
            },
        );
    }
    let retention = document
        .get("backup_retention_days")
        .and_then(Value::as_i64)
        .unwrap_or(7);
    let _ = ctx.ui_set_text(
        &setting_path("stable_setting_retention", "value"),
        &format!("{retention} days"),
    );
}

fn build_identity() -> String {
    format!(
        "Better Mod Menu {MOD_VERSION} initialized (Stable ABI {}, source {BUILD_REVISION}, built {BUILD_TIMESTAMP})",
        ABI_LEVEL
    )
}

fn init(host: &StableHost) -> StableMod {
    host.log(LogLevel::Info, &build_identity());
    let shared = Arc::new(SharedState::new(scan_installed_mods()));
    let mut registration = StableMod::new(MOD_ID);
    registration.set_extension(BetterModMenu {
        shared,
        handlers_registered: AtomicBool::new(false),
        last_search: Mutex::new(String::new()),
        last_selected: AtomicUsize::new(usize::MAX),
        last_key_mask: AtomicUsize::new(0),
    });
    registration
}

declare_stable_mod!(init);

#[cfg(windows)]
fn current_key_mask() -> usize {
    let keys = [
        KEY_UP, KEY_DOWN, KEY_HOME, KEY_END, KEY_E, KEY_D, KEY_ESCAPE,
    ];
    keys.into_iter().enumerate().fold(0, |mask, (index, key)| {
        if unsafe { GetAsyncKeyState(key) } < 0 {
            mask | (1 << index)
        } else {
            mask
        }
    })
}

#[cfg(not(windows))]
fn current_key_mask() -> usize {
    0
}

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(virtual_key: i32) -> i16;
}

#[cfg(windows)]
fn open_https_url(url: &str) -> bool {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    if !url.starts_with("https://steamcommunity.com/") {
        return false;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            window: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_command: i32,
        ) -> *mut c_void;
    }
    let wide = |value: &str| {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let operation = wide("open");
    let file = wide(url);
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        ) as isize
            > 32
    }
}

#[cfg(not(windows))]
fn open_https_url(_url: &str) -> bool {
    false
}

#[cfg(windows)]
fn request_clean_restart() -> bool {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const WM_CLOSE: u32 = 0x0010;
    #[link(name = "user32")]
    extern "system" {
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
    let title = "Teamfight Manager 2\0".encode_utf16().collect::<Vec<_>>();
    let window = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
    !window.is_null() && unsafe { PostMessageW(window, WM_CLOSE, 0, 0) } != 0
}

#[cfg(not(windows))]
fn request_clean_restart() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::{
        build_identity, set_enabled_in_document, visible_indices_for, ModEntry, ModInfo,
        SourceFilter, BUILD_REVISION, MOD_VERSION,
    };
    use serde_json::json;
    use std::path::PathBuf;

    fn entry(name: &str, author: &str, workshop: bool) -> ModEntry {
        ModEntry {
            info: ModInfo {
                mod_id: name.to_lowercase(),
                name: name.to_owned(),
                author: author.to_owned(),
                version: "1.0.0".to_owned(),
                description: String::new(),
                dependencies: Vec::new(),
            },
            root: PathBuf::new(),
            workshop,
            enabled: true,
        }
    }

    #[test]
    fn filters_by_search_and_source() {
        let local = entry("Intro Skip", "MadManPetr1", false);
        assert!(local.matches("intro", SourceFilter::All));
        assert!(local.matches("madman", SourceFilter::Local));
        assert!(!local.matches("intro", SourceFilter::Workshop));
    }

    #[test]
    fn visible_slot_mapping_uses_the_active_search_query() {
        let mods = vec![
            entry("Alpha", "Other", false),
            entry("Intro Skip", "MadManPetr1", false),
            entry("Workshop Match", "MadManPetr1", true),
        ];

        assert_eq!(
            visible_indices_for(&mods, "intro", SourceFilter::All),
            vec![1]
        );
        assert_eq!(
            visible_indices_for(&mods, "madman", SourceFilter::Workshop),
            vec![2]
        );
    }

    #[test]
    fn updates_enabled_ids_without_touching_other_config() {
        let mut document = json!({"enabled_mods":["a","b"],"other":42});
        set_enabled_in_document(&mut document, &["b".to_owned()], false).unwrap();
        assert_eq!(document, json!({"enabled_mods":["a"],"other":42}));
        set_enabled_in_document(&mut document, &["c".to_owned()], true).unwrap();
        assert_eq!(document, json!({"enabled_mods":["a","c"],"other":42}));
    }

    #[test]
    fn build_identity_reports_version_abi_and_revision() {
        let identity = build_identity();
        assert!(identity.contains(MOD_VERSION));
        assert!(identity.contains("Stable ABI"));
        assert!(identity.contains(BUILD_REVISION));
    }
}
