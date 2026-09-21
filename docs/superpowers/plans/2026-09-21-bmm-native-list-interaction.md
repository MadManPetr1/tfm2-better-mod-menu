# Better Mod Menu Native List Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore reliable native wheel scrolling and accurate row controls without losing keyboard-selected-row visibility.

**Architecture:** A disposable in-game probe first tests whether a Stable API `selectable` in a native `scroll_view` scrolls into view when selected programmatically. Only if it passes, replace the recycled ten-slot list with stable mod-ID-bound rows and move click handlers onto visible controls. Pure list identity/navigation helpers remain independent of the game UI; existing catalog and mutation paths remain unchanged.

**Tech Stack:** Rust, `mod-api-stable` (TFM2 `0.6.0+`), the game's `.ui` template format, PowerShell build/install scripts, in-game interaction testing.

**Spec:** `docs/superpowers/specs/2026-09-21-bmm-native-list-interaction-design.md`

## Global Constraints

- Target TFM2 `0.6.0+` and the public Stable API only; no private game ABI, Windows mouse hook, or synthetic wheel injection.
- Keep the existing two-column layout, font assets, colors, fixed right details panel, and footer.
- Preserve the Phase 3 catalog, enabled-state writes, restart detection, and all-installed bulk actions.
- No generic manifest settings, profile cards, status overhaul, dev inspector, release, or publish work in this plan.
- The selected row **must** enter the visible list viewport through proven, supported UI behavior; if the probe fails, stop without replacing the list.
- No live-game file replacement without prior exact-file backup, hashes, a traceable build, and a rollback check.

## File map

- `src/list_state.rs` (new only after the probe passes): pure row-ID encoding, selection reconciliation, and navigation; unit tests live beside these helpers.
- `src/lib.rs`: owns the Stable API lifecycle, dynamic row synchronization, click registration, and existing toggle/restart behavior. Remove `page_start` and ten-slot event mapping only after their replacement works.
- `ui/layout/better_mod_menu_runtime.ui`: change only the left list viewport and obsolete custom scroll track; keep details and footer outside the viewport.
- `scripts/install_local.ps1`, `build_local.ps1`, and `scripts/verify_install.ps1`: reuse unchanged to build and stage a traceable artifact. Do not change release scripts.
- `docs/superpowers/plans/2026-09-21-bmm-native-list-interaction.md`: execution checkboxes only; no separate notes file or shipping probe assets.

## Review Focus

1. A mod ID containing punctuation must produce a unique, valid UI node ID; Task 2 tests byte encoding and collision resistance.
2. A stale click callback after a filter change must not toggle a different mod; Task 2 tests ID resolution against the current visible set.
3. Zero search results must not underflow navigation or show stale details; Task 2 tests empty selection reconciliation and Task 4 checks the rendered empty state.
4. Clicking a toggle must retain that mod as selected without rebuilding or scrolling the viewport; Task 3 adds a targeted unit test and Task 4 tests it in-game.
5. The last of 20+ rows must remain reachable and visible by both wheel and keyboard while the footer stays fixed; Tasks 1 and 4 test this in-game.

---

### Task 1: Prove the Stable API can support the required native interaction

**Files:**
- Temporary modify, then discard: `src/lib.rs`, `ui/layout/better_mod_menu_runtime.ui` in an isolated probe worktree
- Temporary create, then remove: exact `mods/bmm_native_qa_01` through `mods/bmm_native_qa_20` directories in the game root
- Read: `mod-sdk-stable/mod-api-stable/src/client_ctx.rs:379-405`, `src/lib.rs:167-216`, `ui/layout/better_mod_menu_runtime.ui:370-400`

**Interfaces:**
- Consumes: `StableClient::ui_spawn_source`, `ui_set_selectable_selected`, `ui_node_rect`, `ui_contents_rect`, and `ui_register_click`.
- Produces: a recorded go/no-go observation for wheel containment, visible-button hit regions, and keyboard selection auto-scroll. No probe source is carried into the product branch.

- [ ] **Step 1: Protect the live baseline and create the isolated probe checkout.**

Read the `superpowers:using-git-worktrees` skill before creating the probe worktree. Record `git rev-parse HEAD`, `git status --porcelain=v1`, the current live DLL/UI paths, and SHA-256 hashes. Confirm the game is closed before replacing any live file. Copy the exact live DLL, build manifest, and affected `.ui` file to a uniquely named temporary backup directory. Record a copy and hash of `config/game/mods.json`; do not restore it blindly over concurrent user changes.

```powershell
$qaGame = 'D:\Game Clients\Steam\steamapps\common\Teamfight Manager2'
$qaMod = Join-Path $qaGame 'mods\tfm2_better_mod_menu'
$qaBackup = Join-Path $env:TEMP ('bmm-native-probe-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $qaBackup | Out-Null
@('tfm2_better_mod_menu.dll','tfm2_better_mod_menu.build.json','ui\layout\better_mod_menu_runtime.ui') |
  ForEach-Object { Get-FileHash -LiteralPath (Join-Path $qaMod $_) -Algorithm SHA256 }
```

- [ ] **Step 2: Prepare 20 isolated metadata-only fixtures.**

First verify none of the exact fixture directories exists. Use an `apply_patch`-created temporary script in the probe worktree to generate the 20 `mod.mod_info` files with IDs `bmm_native_qa_01` through `bmm_native_qa_20`, distinct names, author `BMM QA`, version `1.0.0`, and no DLL. The script must refuse existing directories and record every created absolute path. Do not subscribe to Workshop items or touch other installed mods. This script is a bulk fixture generator, not a tracked product file.

```powershell
$qaModsRoot = Join-Path $qaGame 'mods'
1..20 | ForEach-Object {
  $id = 'bmm_native_qa_{0:D2}' -f $_
  $path = Join-Path $qaModsRoot $id
  if (Test-Path -LiteralPath $path) { throw "Fixture already exists: $path" }
  New-Item -ItemType Directory -Path $path | Out-Null
  $metadata = [ordered]@{
    mod_id = $id; name = ('BMM Native QA {0:D2}' -f $_); author = 'BMM QA'
    version = '1.0.0'; description = 'Temporary native-list interaction fixture.'
    last_updated = '2026-09-21'; dependencies = @()
  }
  [System.IO.File]::WriteAllText(
    (Join-Path $path 'mod.mod_info'),
    ($metadata | ConvertTo-Json -Depth 4),
    [System.Text.UTF8Encoding]::new($false)
  )
  $path
}
```

- [ ] **Step 3: Make a deliberately minimal probe UI and build it.**

In the probe worktree, change the left list node to a 592-pixel-high `scroll_view` with an `#contents:empty` child using `TopToBottom { spacing: 8px; }`. Spawn one 734 x 52 test row per catalog record beneath `contents`; give it a native `selectable` for selection and visible `button` controls for Enabled/Disabled, without transparent overlays. Start with this minimal probe row and the existing game font; if the template parser rejects `selectable` as a child runner, record that as a failed capability rather than guessing private syntax. Check whether spawning and `ui_set_selectable_selected` return `true`, plus `ui_node_rect` of the selected row and `ui_contents_rect` of the viewport before and after moving to the bottom. Keep the right panel and footer untouched.

```text
probe_row_19:color {
  width: 734px; height: 52px; color: #1d1f2cff;
  #focus:selectable { width: 508px; height: 52px; text: "BMM Native QA 20"; }
  #enable_button:button { x: 508px; y: 8px; width: 82px; height: 36px; source: ""; color: #145648ff; }
  #disable_button:button { x: 594px; y: 8px; width: 82px; height: 36px; source: ""; color: #4a171cff; }
}
```

```rust
let selected_path = format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_rows.contents.probe_row_19.focus");
let before = ctx.ui_node_rect(&selected_path);
let accepted = ctx.ui_set_selectable_selected(&selected_path, true);
let viewport = ctx.ui_contents_rect(&format!("{ROOT}.bmm_menu_grid.bmm_library_panel.bmm_mod_rows"));
eprintln!("native-list probe: accepted={accepted} before={before:?} viewport={viewport:?}");
// Read ui_node_rect again in the next post_update after layout has advanced.
```

Temporarily route the existing `handle_keyboard` Up/Down/Home/End result to the corresponding `probe_row_{index}.focus` path and call `ui_set_selectable_selected` there; do not infer keyboard behavior from a one-time forced selection of row 19. If `eprintln!` is not present in the game log, put the probe result in the existing header summary via `ui_set_text` and record the visible result.

Use `./build_local.ps1 -SdkDir 'D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable'`, then create `$qaStage = Join-Path $env:TEMP ('bmm-native-stage-' + [guid]::NewGuid().ToString('N'))`. Stage with `scripts/install_local.ps1 -SkipBuild -DestinationDir $qaStage`, verify that staged artifact, then copy only the staged DLL, build manifest, and changed runtime `.ui` file into the backed-up live mod directory. Do not commit the probe edits.

- [ ] **Step 4: Exercise the three observations in-game and decide.**

Steam-launch the game. With at least 20 fixtures visible, wheel over the list and then over the details/footer. Click just inside and just outside each visible control's drawn rectangle. Use Up/Down and Home/End to select off-screen rows and compare their reported rects against the viewport contents rect; visually confirm the selected row is actually visible. Filter the list while scrolled to the bottom, then select the first remaining row and confirm it becomes visible. Capture concise pass/fail observations and any game log errors. If any required observation fails, restore the exact backed-up files after closing the game, clean only the recorded fixture paths after validating them, verify original hashes, and **stop the plan**. Report the SDK limitation; do not start Task 2.

- [ ] **Step 5: Restore the baseline even on probe success.**

Close the game and restore only the exact backed-up live files. Remove only probe-created fixtures after confirming each path is inside `$qaModsRoot` and contains only the expected metadata file. Compare live hashes with the recorded originals and confirm the source branch is still clean. Record the proven selectable/scroll behavior for Task 3. The probe build and fixtures are not committed or shipped.

### Task 2: Add pure identity and navigation logic

**Files:**
- Create: `src/list_state.rs`
- Modify: `src/lib.rs:20-40`, `src/lib.rs:1018-1070`

**Interfaces:**
- Consumes: filtered catalog indices (`&[usize]`) and each `ModRecord.identity.mod_id`.
- Produces: `row_node_id(&str) -> String`, `reconcile_selection(&[usize], usize) -> Option<usize>`, `navigation_target(&[usize], usize, NavAction) -> Option<usize>`, and `resolve_visible_id(&[ModRecord], &[usize], &str) -> Option<usize>`.

- [ ] **Step 1: Write failing unit tests in `src/list_state.rs` and declare the module.**

```rust
#[test]
fn row_ids_are_safe_and_distinct() {
    assert_eq!(row_node_id("a-b"), "mod_612d62");
    assert_ne!(row_node_id("a-b"), row_node_id("a_b"));
}

#[test]
fn empty_and_removed_selection_reconcile() {
    assert_eq!(reconcile_selection(&[], 4), None);
    assert_eq!(reconcile_selection(&[2, 5], 4), Some(2));
    assert_eq!(reconcile_selection(&[2, 5], 5), Some(5));
}

#[test]
fn navigation_clamps_at_edges() {
    assert_eq!(navigation_target(&[2, 5], 2, NavAction::Up), Some(2));
    assert_eq!(navigation_target(&[2, 5], 2, NavAction::End), Some(5));
    assert_eq!(navigation_target(&[], 2, NavAction::Down), None);
}

#[test]
fn stale_or_hidden_id_is_not_a_click_target() {
    let mods = vec![ModRecord::test_record("a", "A", "X"),
                    ModRecord::test_record("b", "B", "X")];
    assert_eq!(resolve_visible_id(&mods, &[1], "a"), None);
    assert_eq!(resolve_visible_id(&mods, &[1], "b"), Some(1));
}
```

- [ ] **Step 2: Run the focused tests and confirm failure.**

Run `cargo test --locked list_state`. Expected: compile failure because the declared functions and `NavAction` do not exist yet.

- [ ] **Step 3: Implement the minimal pure helpers.**

```rust
#[derive(Clone, Copy)]
pub(crate) enum NavAction { Up, Down, Home, End }

pub(crate) fn row_node_id(id: &str) -> String {
    let mut result = String::from("mod_");
    for byte in id.as_bytes() { result.push_str(&format!("{byte:02x}")); }
    result
}

pub(crate) fn reconcile_selection(visible: &[usize], selected: usize) -> Option<usize> {
    visible.contains(&selected).then_some(selected).or_else(|| visible.first().copied())
}

pub(crate) fn navigation_target(visible: &[usize], selected: usize, action: NavAction) -> Option<usize> {
    let current = visible.iter().position(|index| *index == selected).unwrap_or(0);
    let next = match action {
        NavAction::Up => current.saturating_sub(1),
        NavAction::Down => (current + 1).min(visible.len().checked_sub(1)?),
        NavAction::Home => 0,
        NavAction::End => visible.len().checked_sub(1)?,
    };
    visible.get(next).copied()
}

pub(crate) fn resolve_visible_id(mods: &[ModRecord], visible: &[usize], id: &str) -> Option<usize> {
    visible.iter().copied().find(|index| mods.get(*index).is_some_and(|m| m.identity.mod_id == id))
}
```

Import `crate::model::ModRecord` in `src/list_state.rs`; the test module imports the four helpers, `NavAction`, and `ModRecord`. The pure helpers do not read the filesystem or call the Stable API.

- [ ] **Step 4: Run `cargo test --locked list_state` and `cargo fmt --check`; fix only any failures in these helpers.** Expected: all four tests pass.
- [ ] **Step 5: Commit only `src/list_state.rs` and its `src/lib.rs` module declaration.** Commit message: `Add stable mod-row identity and navigation helpers`.

### Task 3: Replace slot-based list and transparent row hitboxes

**Files:**
- Modify: `src/lib.rs:40-445`, `src/lib.rs:600-700`, `src/lib.rs:1018-1070`
- Modify: `ui/layout/better_mod_menu_runtime.ui:370-400`
- Test: `src/lib.rs` unit tests and `src/list_state.rs` unit tests

**Interfaces:**
- Consumes: Task 2 helpers; the Task 1-proven `ui_set_selectable_selected` behavior; existing `filtered_indices`, `set_entry_enabled`, and `set_all_enabled`.
- Produces: native list viewport with stable, ID-bound visible controls and keyboard selection synchronized to native scrolling.

- [ ] **Step 1: Add a failing state-transition test.**

Extract a small `select_then_toggle(shared: &SharedState, id: &str, enabled: bool)` wrapper that resolves `id` against the current visible set, sets `selected`, then calls the existing `set_entry_enabled`. Unit-test the no-write branch (requested state already active) to confirm selection is retained even when no file mutation is needed. Keep the actual JSON write path covered by existing `src/io.rs` tests.

```rust
#[test]
fn selecting_an_already_disabled_mod_does_not_clear_selection() {
    let shared = SharedState::new(vec![ModRecord::test_record("a", "A", "X"),
                                       ModRecord::test_record("b", "B", "X")]);
    select_then_toggle(&shared, "b", false);
    assert_eq!(shared.selected.load(Ordering::Acquire), 1);
}
```

Run `cargo test --locked selecting_an_already_disabled_mod_does_not_clear_selection`. Expected: compile failure until the wrapper exists. `ModRecord::test_record` starts disabled, so requesting `false` is the no-write branch.

- [ ] **Step 2: Convert only the left viewport template.**

Replace `#bmm_mod_rows:empty` with a 734 x 592 `scroll_view`, `speed: 80`, `bar_width: 4`, and `#contents:empty` using `TopToBottom { spacing: 8px; }`. Remove `#bmm_mod_scroll_track` and `#bmm_mod_scroll_thumb` after verifying the native bar in-game. Do not move the details panel or footer. Match the probe's known-good syntax exactly.

```text
#bmm_mod_rows:scroll_view {
  x: 0px; y: 100px; width: 734px; height: 592px;
  speed: 80; bar_width: 4;
  #contents:empty {
    width: 734px; height: auto;
    child_type: TopToBottom { spacing: 8px; }
  }
}
```

- [ ] **Step 3: Spawn stable rows only when the filtered ID set changes.**

Add `last_visible_ids: Mutex<Vec<String>>` and `pending_focus: Mutex<Option<String>>` to `BetterModMenu`, initialized empty in `init` and cleared when `body.mods_popup` disappears. On open or search/filter change, remove prior row nodes from `bmm_mod_rows.contents`, spawn all current rows with `row_node_id(mod_id)`, and register click callbacks that capture the original mod ID. Do not remove/recreate rows for selection, a single toggle, or bulk toggles. Check each `ui_spawn_source` and `ui_register_click` return value; on failure emit a diagnostic with the path, keep the baseline build available for rollback, and do not claim acceptance.

```rust
let ids: Vec<String> = visible.iter().map(|&i| mods[i].identity.mod_id.clone()).collect();
if ids != *last_visible_ids {
    for old in last_visible_ids.iter() { let _ = ctx.ui_remove_node(&row_path(old, "")); }
    for id in &ids {
        let path = row_path(id, "");
        if !ctx.ui_spawn_source(&contents, &row_source(id)) { eprintln!("BMM row spawn failed: {path}"); }
        if !register_row_actions(ctx, Arc::clone(&shared), id.clone()) {
            eprintln!("BMM row click registration failed: {path}");
        }
    }
    *last_visible_ids = ids;
}
```

The `row_path` interface changes to `row_path(mod_id: &str, child: &str) -> String`, calling `row_node_id(mod_id)` internally. `register_row_actions(ctx: &mut StableClient<'_>, shared: Arc<SharedState>, id: String) -> bool` registers the selection and both toggle buttons. It resolves the captured ID through `resolve_visible_id` at click time so removed/hidden mods cannot receive stale events. `select_then_toggle(shared: &SharedState, id: &str, enabled: bool)` sets selection before calling the existing `set_entry_enabled`. Keep `set_all_enabled` unchanged and independent of the visible ID list. A spawn/registration failure writes a non-fatal diagnostic to the menu header as well as stderr; do not silently hide an incomplete row.

- [ ] **Step 4: Make the drawn surfaces the event surfaces.**

Use the successful probe's visual-button syntax. Preserve 734 x 52 row size, name/author/source text positions, 82 x 36 toggle sizes, 4-pixel toggle separation, current green/red colors, and font assets. The 508-pixel selection area and the two toggle buttons are disjoint. Remove `select_hitbox`, `enable_hitbox`, and `disable_hitbox`; register clicks on the visible `selectable`/buttons themselves. Update colors on those same nodes when selected/enabled state changes. The clickable rect must be measured with `ui_node_rect` during Task 4.

```rust
let id_for_enable = id.clone();
let enable_path = row_path(&id, "enable_button");
let ok = ctx.ui_register_click(&enable_path, "", move |_| {
    select_then_toggle(&shared, &id_for_enable, true);
});
if !ok { log_registration_failure(&enable_path); }
```

- [ ] **Step 5: Synchronize navigation through the proven native selection behavior.**

Use `navigation_target` for Up/Down/Home/End and `reconcile_selection` for search/filter. In `post_update`, read search/filter, synchronize row children first, handle keyboard second, render row state third, then apply `pending_focus` to the selected row after the newly spawned nodes exist. If the probe showed layout needs one frame, retain `pending_focus` until the following `post_update` rather than calling a missing node. Clear the prior selectable state only if the probe demonstrated this is required. E/D still call `set_entry_enabled` on the selected catalog index. Search/filter changes re-create the visible set and reset the native viewport through the proven probe behavior; ordinary selection/toggle renders do not rebuild it. For empty results, store `usize::MAX` as no selection, show the existing empty-list label, set the right title to `No mod selected`, clear the right metadata/description text, hide thumbnail/banner, and skip selectable calls. When results return, `reconcile_selection` chooses the first visible mod.

```rust
if let Some(next) = navigation_target(&visible, selected, action) {
    shared.selected.store(next, Ordering::Release);
    let path = row_path(&mods[next].identity.mod_id, "focus");
    *pending_focus.lock().unwrap() = Some(path);
    shared.dirty.store(true, Ordering::Release);
}
```

After row synchronization and rendering, consume `pending_focus` by calling `ctx.ui_set_selectable_selected(&path, true)` and leave it pending only when the probe proved that a newly spawned node requires the next frame. On a persistent `false` return, show the non-fatal header diagnostic and fail the in-game acceptance test.

- [ ] **Step 6: Run focused and full checks, then commit.**

Run `cargo fmt --check`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and `./scripts/test_foundation.ps1 -SdkDir 'D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable'`. Fix only interaction-related failures. Commit `src/lib.rs`, `src/list_state.rs`, and the one runtime `.ui` file as `Use native scrolling and visible mod-row controls`.

### Task 4: Validate in-game at realistic scale and leave a clean live state

**Files:**
- No new source files unless a specific Task 3 regression is found
- Reuse: `build_local.ps1`, `scripts/install_local.ps1`, `scripts/verify_install.ps1`

**Interfaces:**
- Consumes: Task 3 traceable build and Task 1 fixture procedure.
- Produces: observed acceptance results, restored user configuration, removed QA fixtures, and a live DLL whose build manifest matches the tested revision.

- [ ] **Step 1: Verify and stage the committed build.**

Confirm clean `git status`, record `git rev-parse HEAD`, run `./build_local.ps1 -SdkDir 'D:\Game Clients\Steam\steamapps\common\Teamfight Manager2\mod-sdk-stable'`, create a fresh `$qaStage` under `$env:TEMP` as in Task 1, stage through `scripts/install_local.ps1 -SkipBuild -DestinationDir $qaStage`, and run `scripts/verify_install.ps1 -DestinationDir $qaStage`. Compare DLL SHA-256 with the build manifest. Back up exact live files and `config/game/mods.json` before installing only the staged runtime artifacts needed for this change.

- [ ] **Step 2: Recreate the 20 QA fixtures and run the acceptance matrix.**

Use the same guarded, recorded fixture paths as Task 1. Steam-launch the game. Test wheel over list versus details/footer; top/middle/bottom mouse targets including button edges; keyboard Up/Down/Home/End with off-screen selection; search and all three source filters, including a no-results search that clears stale details; ordinary selection/toggle without scroll reset; E/D; Enable All/Disable All including preservation of BMM; restart indication; Escape close and Mods reopen; Steam Workshop button. Record screenshots and relevant log lines for the selected bottom row, scrollbar, and fixed footer. A missing keyboard-selected row, overlap click, crash, or changed wrong mod fails acceptance.

- [ ] **Step 3: Clean up without overwriting unrelated data.**

Close the game. Remove only the 20 exact QA directories after confirming their resolved paths are children of the intended `mods` directory and contain only generated metadata. Compare the current `mods.json` against the backup and expected test mutations. If the only changes are test-driven enabled IDs, restore the exact backup; if any unrelated edit is present, stop and report it rather than overwrite. Confirm the live DLL and build manifest match the tested commit and that no probe files or QA fixture directories remain. On failure, restore the backed-up live runtime files and verify their original hashes.

- [ ] **Step 4: Report verified behavior and limitations.**

Report the source revision, automated checks, actual in-game observations, fixture cleanup and configuration status, and any remaining SDK or styling limitation. Do not merge, publish, or claim release readiness from this interaction phase alone.
