# Better Mod Menu manifest

Place `better_mod_menu.json` beside `mod.mod_info`. The manifest is optional:

- Without it, Better Mod Menu shows the metadata supplied by `mod.mod_info`.
- With it, declared `display` fields replace the corresponding game metadata.
- Omitted `display` fields continue to use `mod.mod_info`; values are never duplicated.
- A root `thumbnail.png` replaces the fallback cog.
- A root `banner.png` is shown above the Overview text. `assets/banner.png` is also accepted.

Use `schema_version: 1`, the same `mod_id` and display `name` as the mod, and at most seven controls.

Add the same optional `category` text to adjacent controls to render a compact
section heading. Categories are displayed in manifest order.

```json
{
  "$schema": "./better_mod_menu.schema.json",
  "schema_version": 1,
  "mod_id": "example_mod",
  "name": "Example Mod",
  "storage": "game_data",
  "settings_file": "settings.json",
  "actions_file": "better_mod_menu.actions.json",
  "display": {
    "summary": "A cleaner summary for Better Mod Menu."
  },
  "controls": [
    {
      "type": "toggle",
      "key": "enabled",
      "label": "Feature",
      "category": "General",
      "description": "Enable the feature.",
      "default": true
    },
    {
      "type": "choice",
      "key": "mode",
      "label": "Mode",
      "options": [
        { "label": "Safe", "value": "safe" },
        { "label": "Fast", "value": "fast" }
      ]
    },
    {
      "type": "button",
      "action": "rebuild",
      "label": "Rebuild cache",
      "button_label": "Rebuild"
    }
  ]
}
```

`storage` may be `mod` or `game_data`. `game_data` resolves below
`%APPDATA%/TeamSamoyed/TeamfightManager2/data/<mod_id>`; `mod` resolves below
the installed mod directory. Paths must be relative and cannot contain `..`.

Toggle and choice controls update the JSON settings file immediately while
preserving unknown top-level keys. Button controls write
`{"action":"..."}` to the action file.

`file_cards` renders up to five compact dated file panels. Its `directory` is
relative to the game's data directory. Selecting a card writes its zero-based
`index` with the action:

```json
{
  "type": "file_cards",
  "action": "import_backup",
  "label": "Recent verified backups",
  "description": "Select one to import it.",
  "directory": "example_backups",
  "filename_contains": "__before_mod_load_",
  "extension": "data",
  "limit": 5
}
```

The receiving mod reads `{"action":"import_backup","index":0}` and remains
responsible for validating and performing the operation.
