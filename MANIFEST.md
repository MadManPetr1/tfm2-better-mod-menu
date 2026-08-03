# Better Mod Menu manifest reference

Place `better_mod_menu.json` beside `mod.mod_info`. The manifest is optional;
without it, Better Mod Menu uses the metadata supplied by the game.

## Root fields

| Field | Purpose |
| --- | --- |
| `$schema` | Public schema URL for editor validation |
| `schema_version` | Must be `1` |
| `mod_id` | Must match the installed mod ID |
| `name` | Must match the mod name |
| `storage` | `mod` or `game_data` |
| `settings_file` | Relative JSON settings path |
| `actions_file` | Optional relative action-request path |
| `display` | Optional title, author, version, and summary overrides |
| `controls` | Up to seven controls in display order |

Omitted display fields fall back to `mod.mod_info`. Installed source and
dependencies always come from the game and cannot be overridden.

## Storage

`game_data` resolves below:

```text
%APPDATA%/TeamSamoyed/TeamfightManager2/data/<mod_id>
```

`mod` resolves below the installed mod folder. Paths must be relative and may
not contain `..`, a drive prefix, or a rooted path.

Toggle and choice controls update the settings JSON immediately while
preserving unknown top-level keys.

## Controls

### Toggle

```json
{
  "type": "toggle",
  "key": "enabled",
  "label": "Feature",
  "category": "General",
  "description": "Enable the feature.",
  "default": true
}
```

### Choice

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

### Button action

```json
{
  "type": "button",
  "action": "rebuild",
  "label": "Rebuild cache",
  "button_label": "Rebuild"
}
```

Buttons write an action request; the receiving mod remains responsible for
validation and execution.

### File cards

```json
{
  "type": "file_cards",
  "action": "import_backup",
  "label": "Recent verified backups",
  "description": "Select one to import it.",
  "directory": "example_backups",
  "filename_contains": "__verified_",
  "extension": "data",
  "limit": 5
}
```

The directory is relative to game data. Selecting a card writes the action and
its zero-based `index`; Better Mod Menu never interprets or imports the file.

## Categories and artwork

Adjacent controls with the same optional `category` render beneath one compact
heading. Categories appear in manifest order.

`thumbnail.png` replaces the fallback header icon. `banner.png` appears above
the Overview text, with `assets/banner.png` accepted as a fallback.

For copy-ready starting points, use
[better_mod_menu.json.example](better_mod_menu.json.example) and the
[public schema](better_mod_menu.schema.json).
