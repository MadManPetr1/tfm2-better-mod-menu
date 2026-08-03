# Better Mod Menu: modder guide

Better Mod Menu does not replace the game's mod API. Your mod still loads,
saves, and validates its own behavior. Better Mod Menu only provides an
optional visual bridge for settings and actions.

A mod needs at most two optional Better Mod Menu integration files beside its
normal `mod.mod_info`:

- `better_mod_menu.json` for richer display metadata, settings, and actions.
- `better_mod_menu_profile.json` for the optional author profile card.

Use either file independently or both together. Schema and example files stay
in the Better Mod Menu repository/release and do not need to be copied into
your mod.

## Five-minute setup

1. Keep a normal `mod.mod_info`. This remains the authoritative source for the
   mod name, version, author, description, and dependencies.
2. Add `better_mod_menu.json` beside it.
3. Set `mod_id` and `name` to the same values used by your mod.
4. Add the controls you want Better Mod Menu to render.
5. Read your settings JSON periodically if settings should apply without a
   restart.

If Better Mod Menu is not installed, your mod continues to work normally.

For the easiest start, copy `better_mod_menu.json.example` into your mod root,
rename it to `better_mod_menu.json`, and edit the example values. Keep the
`$schema` line points to the public schema, so editors such as VS Code can
suggest fields and flag typing mistakes without another local file. The
profile works the same way: copy
`better_mod_menu_profile.json.example`, rename it to
`better_mod_menu_profile.json`, and edit it. Both files belong beside
`mod.mod_info`.

## Smallest useful example

```json
{
  "$schema": "https://raw.githubusercontent.com/MadManPetr1/tfm2-better-mod-menu/main/better_mod_menu.schema.json",
  "schema_version": 1,
  "mod_id": "my_mod",
  "name": "My Mod",
  "storage": "game_data",
  "settings_file": "settings.json",
  "controls": [
    {
      "type": "toggle",
      "key": "enabled",
      "label": "Enable feature",
      "category": "General",
      "description": "Turns the feature on or off.",
      "default": true
    }
  ]
}
```

With `storage: "game_data"`, the toggle is saved to:

```text
%APPDATA%/TeamSamoyed/TeamfightManager2/data/my_mod/settings.json
```

The resulting file is ordinary JSON:

```json
{
  "enabled": false
}
```

Unknown top-level keys are preserved, so your mod may safely keep additional
state in the same file.

## Controls

### Toggle

Use for Boolean settings. Better Mod Menu renders the native-styled
`ON | OFF` control and saves immediately.

```json
{
  "type": "toggle",
  "key": "show_overlay",
  "label": "Show overlay",
  "category": "Interface",
  "description": "Display the match overlay.",
  "default": true
}
```

### Choice

Use for a short list of predefined values. Values may be strings, numbers,
Booleans, or JSON values.

```json
{
  "type": "choice",
  "key": "detail_level",
  "label": "Detail level",
  "category": "Interface",
  "options": [
    { "label": "Compact", "value": "compact" },
    { "label": "Full", "value": "full" }
  ]
}
```

### Button action

Buttons do not execute mod code directly. They create a small action request
that your DLL consumes and deletes.

```json
{
  "type": "button",
  "action": "rebuild_cache",
  "label": "Rebuild cache",
  "category": "Maintenance",
  "description": "Regenerate cached data.",
  "button_label": "Rebuild"
}
```

Default action file:

```text
better_mod_menu.actions.json
```

Payload:

```json
{
  "action": "rebuild_cache"
}
```

Your mod should treat action files as untrusted input: validate the action,
perform the operation, report errors in its own log, and delete the request.

### File cards

Use for a small set of recent files such as verified backups. Better Mod Menu
sorts by modified time and renders up to five dated cards.

```json
{
  "type": "file_cards",
  "action": "import_backup",
  "label": "Recent verified backups",
  "category": "Backup and recovery",
  "description": "Select one to import it.",
  "directory": "my_mod_backups",
  "filename_contains": "__verified_",
  "extension": "data",
  "limit": 5
}
```

Selecting the first card produces:

```json
{
  "action": "import_backup",
  "index": 0
}
```

The directory is relative to the game's data directory. Better Mod Menu never
interprets or imports the file itself; that remains your mod's responsibility.

## Categories

Give adjacent controls the same `category` text. Better Mod Menu inserts one
compact heading whenever the category changes. No separate category registry
is required.

Keep categories short and use manifest order:

```text
General
Interface
Backup and recovery
Advanced
```

## Preview artwork

These files are optional and require no manifest fields:

- `thumbnail.png` replaces the fallback cog in the header.
- `banner.png` appears at the top of Overview.
- `assets/banner.png` is accepted as a fallback location.

Without artwork, Better Mod Menu retains its normal fallback presentation.

## Metadata and dependency health

Keep real dependency declarations in `mod.mod_info`. Better Mod Menu uses those
entries to detect missing, disabled, and version-incompatible dependencies.

The manifest `display` object can override title, author, version, and summary.
Source and dependencies always come from the installed mod and `mod.mod_info`.

## Optional author profile

Place `better_mod_menu_profile.json` beside `mod.mod_info`:

```json
{
  "$schema": "https://raw.githubusercontent.com/MadManPetr1/tfm2-better-mod-menu/main/better_mod_menu_profile.schema.json",
  "schema_version": 1,
  "display_name": "MadManPetr1",
  "profile_icon": "profile_icon.png",
  "bio": "Developer and programmer. Modding TFM2, mainly focusing on UX/UI and realism.",
  "links": {
    "github": "MadManPetr1",
    "youtube": "@l95_madmanpetr1",
    "discord": "l95madmanpetr1"
  }
}
```

The bio is limited to 240 characters. Only validated GitHub usernames and
YouTube handles become clickable links. A validated modern Discord username is
shown as a contact line and is copied when clicked; it is not opened as a URL.
Set `profile_icon` to `profile_icon.png` or `profile_icon.jpg` to show that
local file at 32 by 32 pixels. Arbitrary URLs, other filenames, network image
requests, and other providers are not supported. The card is optional and is
not identity verification.

## Safety rules

- Use only relative paths.
- `..`, rooted paths, and drive-prefixed paths are rejected.
- Use no more than seven controls.
- A choice must contain at least one option.
- Mods must validate every requested action themselves.
- Do not require Better Mod Menu for core mod behavior.

For editor validation and the complete field reference, use the public
`better_mod_menu.schema.json` and `better_mod_menu_profile.schema.json` links
already included in the examples. Do not copy the schemas into your mod. The
two `.example` files are copy-ready starting points; a complete production
settings example is also available in Intro Skip's `better_mod_menu.json`.
