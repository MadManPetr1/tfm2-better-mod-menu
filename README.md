# Better Mod Menu

A focused replacement surface for the **Teamfight Manager 2** mod manager.

> [!IMPORTANT]
> Version **0.5.3** is built for Teamfight Manager 2 **0.5.3** on Windows.

Better Mod Menu keeps the game's native mod data and interaction model while
adding faster navigation, richer previews, dependency warnings, and an
optional manifest bridge for mod-owned settings.

## Features

- Search installed mods by name.
- Filter all, native-code, and Workshop mods.
- Navigate and toggle mods with mouse or keyboard.
- Inspect richer metadata, dependency health, thumbnails, and banners.
- Render optional manifest-driven toggles, choices, actions, and file cards.
- Apply compatible setting changes immediately while preserving unknown JSON
  fields.
- Restart the game cleanly when pending mod changes require it.

## Safety and scope

- Better Mod Menu does not replace the game mod API.
- Each mod remains responsible for validating and applying its own behavior.
- Manifest paths are restricted to the declaring mod or its game-data folder.
- Manifest IDs must match the installed mod ID.
- Unsupported or malformed manifests are ignored.
- Career saves are never opened or modified by Better Mod Menu.

## Installation

Download `better-mod-menu-v0.5.3.zip` from
[GitHub Releases](https://github.com/MadManPetr1/tfm2-better-mod-menu/releases).
Do not use GitHub's automatic source-code archive.

Extract the included `mod_menu` folder into:

```text
...\SteamLibrary\steamapps\common\Teamfight Manager2\mods\
```

Confirm this structure:

```text
Teamfight Manager2\mods\mod_menu\mod.mod_info
Teamfight Manager2\mods\mod_menu\mod_menu.dll
Teamfight Manager2\mods\mod_menu\ui\
```

Enable **Better Mod Menu** in the native Mods menu and restart the game.

## Optional mod integration

Mods work normally without a Better Mod Menu manifest. Authors who want rich
settings can place `better_mod_menu.json` beside `mod.mod_info`.

See:

- [Manifest reference](MANIFEST.md)
- [Modder guide](MODDER_GUIDE.md)
- [JSON schema](better_mod_menu.schema.json)

## Building from source

Install the Teamfight Manager 2 `0.5.3` Mod SDK, then run:

```powershell
.\build_local.ps1 -SdkDir "C:\path\to\Teamfight Manager2\mod-sdk-0.5.3"
.\scripts\validate_repo.ps1
.\scripts\package_release.ps1 -SdkDir "C:\path\to\Teamfight Manager2\mod-sdk-0.5.3"
```

## License and attribution

Source code and documentation are licensed under
[GPL-3.0-or-later](LICENSE), with a narrow
[TFM2 linking exception and attribution terms](LICENSE-EXCEPTION.md).
Redistributed versions must keep the source available and preserve the
original author/repository credit. Forks must use their own name and branding.

The bundled status icons are derived from Apache-2.0 licensed icon projects;
see [third-party notices](THIRD_PARTY_NOTICES.md).

This is an independent community mod and is not affiliated with or endorsed by
Team Samoyed.
