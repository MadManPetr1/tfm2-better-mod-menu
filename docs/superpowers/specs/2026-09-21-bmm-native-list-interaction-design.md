# Better Mod Menu: Native List Interaction Recovery

Date: 2026-09-21

Status: Written design for user review; no implementation approved yet

## Purpose and success

Restore the original Better Mod Menu's dependable list interaction on TFM2
`0.6.0+` without reviving the retired private-API implementation. Mouse users
must be able to wheel-scroll long lists and click the visible controls at their
drawn bounds. Keyboard users must be able to navigate the same list without the
selected row disappearing outside the viewport. The established two-column
layout, fonts, colors, fixed details panel, and footer remain intact.

This design covers the next interaction phase only. Generic manifest settings,
profile cards, rich status presentation, and release packaging remain separate
work.

## Baseline and constraints

- A Steam-launched TFM2 `0.6.0` session opened BMM `0.8.0`; selecting a row,
  closing with Escape, and reopening from the title Mods button worked. The
  earlier failed direct launch happened while BMM was disabled and is not
  evidence that BMM caused the startup failure.
- The current list is an `empty` container with ten recycled 734 x 52 rows,
  `page_start`, transparent click overlays, and a manually drawn scrollbar.
  The right-side description already uses a native `scroll_view`.
- The Stable SDK exposes UI node spawning, click registration, property and
  geometry reads, and selected state for `selectable`. It documents no wheel
  event and no readable or writable `scroll_view` offset. No undocumented
  scroll API is assumed.
- Changes to the live game installation need a traceable build, exact backups,
  and an in-game check. The source branch and current live mod copy are not a
  disposable playground.

## Chosen approach

Use the game's native `scroll_view` for the mod-list viewport and visible
`button`/`selectable` runners for row actions where their real interactive
rectangles can be verified. Retain the current rendering style and the
existing catalog, enabled-state mutation, restart detection, search, and filter
paths. Keep the right panel and footer outside the list scroll view.

The alternatives are inferior here: extending the ten-slot/manual-scroll
system would require a wheel signal the public SDK does not expose; a Windows
mouse hook or private game ABI would add fragility and break the Stable API
boundary. Neither is an approved fallback.

### Capability gate before the migration

In an isolated build, create a minimal native-list probe with at least 20
temporary local mod entries. Check these observations in the running game:

1. The wheel moves only the left list while the pointer is over it; rows stay
   clipped to the panel and the details/footer remain fixed.
2. A clickable visible control receives events only inside its drawn bounds;
   its hover/cursor state matches the rest of the menu.
3. Up/Down and Home/End can move selection to a row above or below the current
   viewport **and bring that row into view through supported Stable API/UI
   behavior**. Candidate mechanisms are native focus/selection behavior or a
   documented template/property interaction, but each must be observed rather
   than inferred from source.

If observation 3 fails, do not replace the working keyboard list with a native
scroll view. Stop this phase and report the exact SDK limitation and probe
result. A partially native implementation that strands keyboard selection is
not acceptable. If observation 2 fails, keep the current controls unchanged
and report it rather than merely shifting transparent overlays.

Probe code and generated mod entries are not shipped. Temporary fixture IDs
must be unique; record exact paths before creation and remove only those
fixtures after verifying their paths and contents. Preserve the user's mods,
configuration, saves, and current live DLL. Restore the prior live build after
the probe unless the later implementation passes the acceptance test.

## Interaction model if the gate passes

- The list scroll view owns a stable child per filtered mod (or a supported
  equivalent proven in the probe), identified by mod ID rather than a recycled
  slot number. This keeps click targets bound to the correct mod while scrolling.
- Clicking the non-toggle portion selects the mod and updates details. Clicking
  Enabled or Disabled applies the existing single-mod toggle path and keeps
  that mod selected. The visible button surface owns the click; there is no
  separate transparent hitbox over it. The two toggle regions do not overlap.
- Wheel movement changes only the native viewport. It does not alter selection
  or enabled state. Re-rendering a selected row or a toggle does not reset the
  viewport.
- Up/Down move one filtered result; Home/End move to the first/last result.
  Selection is scrolled into view using only the mechanism proven by the gate.
  E/D continue to act on the selected mod. Empty results disable row navigation
  and show the existing empty state.
- Search or source-filter changes rebuild the visible set, reset scroll to the
  beginning, and select the first matching mod if the previous selection is
  no longer present. If it remains present, selection may stay, but must be
  visible after the reset. No filter may remove installed mods from bulk
  enable/disable behavior.
- Native scrollbar styling should retain the menu's teal accent if the public
  UI styling supports it. A functional native scrollbar takes precedence over
  reproducing the current custom thumb; no white overlay scrollbar or second
  competing scrollbar should remain.

The implementation should make the smallest necessary changes to
`ui/layout/better_mod_menu_runtime.ui` and `src/lib.rs`. It should not refactor
the Phase 3 data core or `src/dev_inspector.rs`. If `selectable` is necessary
for supported focus/scroll behavior, its visual treatment must still match the
existing row and its state must not be confused with enabled/disabled state.

## Failure handling and rollback

- Template spawn or event registration failure leaves the menu usable through
  the existing path in a development build and produces a concise log entry.
  Do not silently ship a list without working keyboard navigation.
- A click that targets a mod removed by a concurrent search/filter refresh is
  ignored safely, never redirected to whatever occupies an old row slot.
- The implementation is staged and built from a clean source revision; keep a
  copy and hash of the prior live DLL and affected UI files. On a failed smoke
  test, restore only those exact files and verify their hashes. Do not alter
  unrelated installed mods or user configuration.

## Verification and acceptance

Automated checks: `cargo fmt --check`, `cargo test --locked`, targeted lint or
foundation checks already used by this branch, and a Stable SDK release build.
Tests should cover filtered-index/selection reconciliation and ensure toggling
does not change the selected mod. Runtime behavior cannot be claimed from unit
tests alone.

In-game smoke test with at least 20 temporary installed mods:

- Wheel over list scrolls it; wheel over details/footer does not scroll list.
- Top, middle, and bottom rows can be selected and toggled with the mouse at
  their visible bounds, without neighboring controls firing.
- Up/Down and Home/End keep selection visible, including near the bottom.
- Search and All/Local/Workshop filters preserve correct row identity and
  reset/reconcile scroll without jumping on ordinary selection or toggles.
- Enable All/Disable All still apply to all installed mods, preserve BMM on
  disable, and update restart state.
- The fixed details panel, footer, Escape close, Mods reopen, and Steam
  Workshop button still work.
- Temporary test mods are removed safely, and the live build matches the
  tested source revision.

This phase is complete only if the capability gate and all applicable smoke
checks pass. Otherwise the deliverable is a documented limitation and intact
working baseline, not a partial release candidate.
