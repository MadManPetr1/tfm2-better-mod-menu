# Better Mod Menu integration guide

Better Mod Menu integration is optional. A normal TFM2 mod works without it,
and Better Mod Menu never replaces your mod's validation or runtime logic.

Using the two public JSON formats does **not** require publishing your mod's
source code or licensing it under the GPL. Your JSON, artwork, DLL, and source
remain yours. The Better Mod Menu license applies only if you copy, modify,
link, or redistribute Better Mod Menu code or binaries.

## Choose what you need

| Goal | Add this beside `mod.mod_info` |
| --- | --- |
| Rich settings, actions, or display text | `better_mod_menu.json` |
| Author bio, icon, and contact links | `better_mod_menu_profile.json` |
| Thumbnail in the mod header | `thumbnail.png` |
| Wide image in the Overview tab | `banner.png` |

Use any combination. The schema files stay in the Better Mod Menu repository;
your mod only ships its own JSON and artwork.

## Settings and actions

1. Copy `better_mod_menu.json.example` into your mod root.
2. Rename it to `better_mod_menu.json`.
3. Match `mod_id` and `name` to `mod.mod_info`.
4. Choose `mod` or `game_data` storage.
5. Add up to seven controls.
6. Let your mod read and validate the resulting settings or action request.

Smallest useful manifest:

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

With `game_data` storage, the setting is written to:

```text
%APPDATA%/TeamSamoyed/TeamfightManager2/data/my_mod/settings.json
```

Unknown top-level keys are preserved. Your mod may keep additional state in
the same JSON file, but it must still validate values before using them.

### Manifest fields

| Field | Purpose |
| --- | --- |
| `$schema` | Enables editor validation and completion |
| `schema_version` | Must be `1` |
| `mod_id` | Must exactly match `mod.mod_info` |
| `name` | Must exactly match `mod.mod_info` |
| `storage` | `mod` or `game_data` |
| `settings_file` | Relative JSON settings path |
| `actions_file` | Optional relative action-request path |
| `display` | Optional title, author, version, and summary overrides |
| `controls` | Up to seven controls in display order |

Omitted display fields fall back to `mod.mod_info`. Source, dependencies, and
enabled state always come from the game and cannot be overridden.

`game_data` stores files below:

```text
%APPDATA%/TeamSamoyed/TeamfightManager2/data/<mod_id>
```

`mod` stores them below the installed mod folder. Paths must be relative and
cannot contain `..`, a drive prefix, or a rooted path.

## Available controls

| Type | Best for | What Better Mod Menu writes |
| --- | --- | --- |
| `toggle` | Boolean settings | The selected Boolean value |
| `choice` | A short predefined list | The selected JSON value |
| `button` | A requested operation | An action name in the action file |
| `file_cards` | Selecting a recent backup or export | An action name and zero-based file index |

Use the same optional `category` text on adjacent controls to create a compact
section heading. Keep categories short and order controls as they should appear.

Toggle:

```json
{
  "type": "toggle",
  "key": "enabled",
  "label": "Feature",
  "description": "Enable the feature.",
  "default": true
}
```

Choice:

```json
{
  "type": "choice",
  "key": "mode",
  "label": "Mode",
  "options": [
    { "label": "Safe", "value": "safe" },
    { "label": "Fast", "value": "fast" }
  ]
}
```

Button:

```json
{
  "type": "button",
  "action": "rebuild_cache",
  "label": "Rebuild cache",
  "button_label": "Rebuild"
}
```

File cards:

```json
{
  "type": "file_cards",
  "action": "import_backup",
  "label": "Recent backups",
  "directory": "backups",
  "filename_contains": "__verified_",
  "extension": "data",
  "limit": 5
}
```

File-card directories are relative to game data. A selection writes the action
and its zero-based file index; Better Mod Menu does not interpret the file.

Buttons and file cards do not execute code. They create a request such as:

```json
{
  "action": "rebuild_cache"
}
```

Your mod must validate, perform, report, and delete that request.

The public [manifest schema](better_mod_menu.schema.json) is the authority for
exact field rules and provides editor completion and validation.

## Author profile

Copy `better_mod_menu_profile.json.example`, rename it to
`better_mod_menu_profile.json`, and keep it beside `mod.mod_info`:

```json
{
  "$schema": "https://raw.githubusercontent.com/MadManPetr1/tfm2-better-mod-menu/main/better_mod_menu_profile.schema.json",
  "schema_version": 1,
  "display_name": "YourName",
  "profile_icon": "profile_icon.png",
  "bio": "A short description of your TFM2 work.",
  "links": {
    "github": "YourGitHubName",
    "discord": "your.discord.username"
  }
}
```

- Bio length is limited to 240 characters.
- Profile icons may be `profile_icon.png` or `profile_icon.jpg`.
- GitHub and YouTube values are validated before becoming links.
- A modern Discord username is shown as copyable contact text.
- Remote images and arbitrary URLs are not loaded.

Use the public [profile schema](better_mod_menu_profile.schema.json) for the
complete field rules.

## Artwork

Artwork needs no manifest field:

- `thumbnail.png` appears in the mod header.
- `banner.png` appears at the top of the Overview tab.
- `assets/banner.png` remains accepted as a fallback.

Without artwork, Better Mod Menu uses its normal fallback presentation.

## Release checklist

- Keep `mod.mod_info` authoritative for dependencies and supported game versions.
- Match the manifest `mod_id` and `name` exactly.
- Use only relative paths; rooted paths, drive prefixes, and `..` are rejected.
- Keep core mod behavior independent of Better Mod Menu.
- Treat settings and action files as untrusted input.
- Test the mod both with and without Better Mod Menu enabled.
- Validate JSON against the public schemas before packaging.

For a complete working integration, see Intro Skip's
`better_mod_menu.json` in its source repository.
