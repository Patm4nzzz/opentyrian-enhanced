<p align="center">
  <img src="marketing/banner.jpg" alt="OpenTyrian2000 Enhanced Edition: Tyrian, the 1995 classic, reborn" width="900">
</p>

<p align="center">
  <b>English</b> · <a href="README.fr.md">Français</a> · <a href="README.es.md">Español</a> · <a href="README.de.md">Deutsch</a> · <a href="README.it.md">Italiano</a> · <a href="README.pt-BR.md">Português</a> · <a href="README.ru.md">Русский</a>
</p>

<p align="center">
  <b>Tyrian 2000 on a new Godot 4 + Rust engine: the full campaign, Endless Arcade, couch co-op, five mod packs and a level editor. Free and open source.</b>
</p>

<p align="center">
  <a href="https://tyrian-enhanced.com/"><b>🌐 tyrian-enhanced.com</b></a> ·
  <a href="#%EF%B8%8F-download"><b>⬇️ Download</b></a> ·
  <a href="#-mod-packs-and-modding">Mods</a> ·
  <a href="https://tyrian-enhanced.com/wiki/">Wiki</a> ·
  <a href="docs/PLAYER-GUIDE.md">Player guide</a>
</p>

<p align="center">
  <img src="marketing/godot/hero.gif" alt="Real gameplay: classic and mod-pack ships fire Thermal Lance, Spore Burst, Lightning Cannon, Frost Shard, Banana Blast, Arc Caster, Starburst and Zica Laser through dense enemy waves" width="720"><br>
  <sub>Real gameplay from the new engine. 🔊 <b><a href="https://youtu.be/THaDU0-e4YY">Watch the trailer with sound</a></b>.</sub>
</p>

## What is this?

Tyrian is the cult 1995 vertical shoot-'em-up by Eclipse Software: you are **Trent Hawkins**, fighter pilot, up against the MicroSol corporation across **five episodes** of frantic dodging, over-the-top weaponry and secret-stuffed levels.

**OpenTyrian2000 Enhanced Edition** plays the complete Tyrian 2000 campaign on a new engine for **Windows, Mac, Linux and Steam Deck**. Play it as you remember it, or switch on Endless waves, couch co-op, mutators and mods you build yourself.

## ✨ What Tyrian 2000 never had

- ♾️ **Endless Arcade** (Endless Mode in the menu) — levels strung into escalating waves, combo multipliers, a top-10 leaderboard and instant retry, for one or two players.
- 👥 **Couch co-op** — two players on two gamepads or a shared keyboard, in the campaign and in Endless.
- 🎲 **Mutators** — optional rule changes you can stack on any run: *Bullet Hell*, *Glass Cannon*, *Mirror Mode* and more.
- 🏆 **Achievements** — browse the list in game with F3, with a notice the moment you unlock one.
- 💥 **Impact effects** — enemies freeze for a beat on heavy hits, bosses go down in a chain of explosions, colored damage numbers pop up. Each one can be turned off.
- 🖼️ **HD (AI) art, sharp pixel art and five CRT filters** — or the original pixels, one key away.
- 🎵 **Enhanced sound engine** — the General MIDI soundtrack with its own SoundFont, next to the original AdLib music.
- 🧩 **Modding system** — add weapons, ships, sidekicks, shields, enemies and level encounters from a folder of JSON and PNG files, no recompiling; the game reloads your mod on the spot (5 sample mod packs included!).
- 🛠️ **A level editor** — place enemies, export to a mod, build a `.zip` and hit **Play from here**.
- 🎮 **Gamepad in every menu**, remappable for both players, with a Steam Deck profile when launched from Steam.
- 🌍 **Multi-language support** — English, French, Spanish, German, Italian, Portuguese.

| | | |
|---|---|---|
| ![Modern explosions and damage numbers over Savara V](marketing/godot/combat.png) | ![The Savara boss in HD art mode](marketing/godot/boss.png) | ![Two Plasma Storm sidekicks flanking the ship](marketing/godot/sidekicks.png) |
| ![Two players in co-op](marketing/godot/two-player-endless.png) | ![Endless Arcade waves](marketing/godot/endless-waves.png) | ![The shop](marketing/godot/shop.png) |

## 🧩 Mod packs and modding

Five packs ship with the game, all off until you turn them on in **Setup → Mods**. They are drawn in the original Tyrian style, sold in every shop, balanced against the original items of the same price, and can all be on together.

| Pack | What it adds |
|---|---|
| **Alien Ship Pack** | 8 ships of Tyrian's two alien races, the Hazudra and the Zica, with banking poses |
| **Bio Ships Pack** | 6 bio-organic ships: carapaces, membrane wings and glowing spore sacs |
| **Alien Weapon Pack** | 6 Hazudra and Zica guns: thermal lances, a crescent fan, homing darts, plasma rings, a lobbed mortar |
| **Bio Weapons Pack** | 6 living green guns: spore bursts, acid globs, piercing thorns, homing larvae |
| **Elements Weapon Pack** | 6 elemental guns: fire, ice that freezes enemies, lightning, rock, acid and wind |

<p align="center">
  <img src="marketing/godot/mod-packs.gif" alt="The Zica Twin Furnace from the Alien Ship Pack firing Alien Weapon Pack guns in an asteroid field" width="528"><br>
  <sub>The Zica Twin Furnace (Alien Ship Pack) with Alien Weapon Pack guns.</sub>
</p>

| | | | | |
|---|---|---|---|---|
| ![Alien Ship Pack in the shop](marketing/godot/alien-ship-pack-shop.webp) | ![Bio Ships Pack in the shop](marketing/godot/bio-ships-pack-shop.webp) | ![Alien Weapon Pack in the shop](marketing/godot/alien-weapon-pack-shop.webp) | ![Bio Weapons Pack in the shop](marketing/godot/bio-weapons-pack-shop.webp) | ![Elements Weapon Pack in the shop](marketing/godot/elements-weapon-pack-shop.webp) |

**Make your own.** A mod is just a folder: a `mod.json` describing what you add, plus optional PNG sprites. Install a `.zip` from **Setup → Mods** and the game reloads it on the spot.

- **Weapons, ships, sidekicks, shields and specials** — sold in the shop, or swapped into your starting loadout.
- **Enemies** — rebalance any original enemy, or create new ones with their own flight paths and turrets.
- **Level encounters** — add enemies to existing levels, written by name: no ID numbers to juggle.
- **Level editor in Godot** — place enemies, export, package and **Play from here**. Start with **[Your first level in 15 minutes](docs/FIRST_LEVEL_15_MIN.md)**.
- **Safe by design** — a broken mod shows a clear error in the menu instead of crashing the game, and a JSON schema gives you autocomplete in your editor.

Not moddable yet: level maps (tiles), music, sound effects and the palette. Full reference: **[MODDING.md](MODDING.md)**.

## ⬇️ Download

> **[v1.0-rc1](https://github.com/Patm4nzzz/opentyrian-enhanced/releases/tag/v1.0-rc1)**, the first release, is a release candidate: the whole game is playable from start to finish. Try it and tell us what you find ([release notes](docs/release-notes-v1.0-rc1.md)).

| Download | Size | |
|---|---:|---|
| `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip` | 704.0 MB | Windows, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz` | 683.4 MB | Linux / Steam Deck, full |
| `OpenTyrian2000-Enhanced-v1.0-rc1-macos-universal.zip` | 727.3 MB | macOS (Intel and Apple Silicon), full, experimental |
| `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64-lite.zip` | 197.3 MB | Windows, lite (no HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64-lite.tar.gz` | 185.6 MB | Linux / Steam Deck, lite |
| `OpenTyrian2000-Enhanced-v1.0-rc1-hd-pack.zip` | 506.5 MB | HD (AI) art for a lite download, Windows or Linux |

- **Windows** — extract the `.zip`, open the `OpenTyrian2000-Enhanced` folder, run `OpenTyrian2000-Enhanced.exe`. If Windows shows "Windows protected your PC", click **More info → Run anyway**; the SHA-256 of each archive is in `SHA256SUMS.txt`.
- **Linux / Steam Deck** — extract the `.tar.gz`, run `./OpenTyrian2000-Enhanced.x86_64`. On Steam Deck, add it to Steam as a non-Steam game so its controller profile applies ([Steam Deck guide](https://tyrian-enhanced.com/steam-deck/)).
- **macOS (experimental):** unzip, then open `OpenTyrian2000 Enhanced.app` (you can move it to Applications). The app is not signed or notarized by Apple yet: the first time, right-click (or Control-click) it, choose **Open**, then **Open** again. If macOS still refuses, run `xattr -cr "/Applications/OpenTyrian2000 Enhanced.app"` in Terminal (with the path of the app). Requires macOS 10.13+ on Intel, macOS 11+ on Apple Silicon.
- Every download includes the game data, the MIDI soundtrack and the five mod packs; the full ones also include the HD (AI) art. Keep the whole folder together.

No installer, no account. Known issue: network play is not available ("Modem/Network Game" is greyed out).

## 🎮 Controls

| | Keyboard | Gamepad |
|---|---|---|
| Move | Arrow keys | Left stick / d-pad |
| Fire | Space | A / RT |
| Rear-gun mode | Enter | B |
| Left / right sidekick | Ctrl / Alt | X / Y |
| Pause / resume | P or F2 | Start / RB |
| In-game menu | Esc | Back / LB |
| In-game help | F1 | — |
| Achievements / statistics (in a level) | F3 / F4 | — |
| Quick save / quick load (campaign) | F5 / F9 | — |
| CRT filter / graphics mode (outside a level) | F2 / F3 | — |

Setup → Key Bindings changes the keys. At the title screen: **M** for side modes and mutators, **2** for two-player Endless, **DESTRUCT** for the mini-game. Everything else is in the **[player guide](docs/PLAYER-GUIDE.md)**.

## 🛠️ Under the hood

- **Rust core** for game logic, file formats and mods; **Godot 4** for rendering, input, audio and the editor.
- **Checked against the original game logic**: recorded runs replay tick for tick in strict tests, and an automated pilot plays the five-episode campaign end to end.
- **Packages tested as shipped**: the Linux package is played under Xvfb with real input, and the exported Windows executable replays a recorded run inside it.
- Build it yourself: **[docs/build.md](docs/build.md)** · design notes: **[docs/01-architecture.md](docs/01-architecture.md)** · tests: **[docs/testing.md](docs/testing.md)**.

## 🙏 Credits & licence

- **Tyrian** and **Tyrian 2000** © Eclipse Software, published by Epic MegaGames; the game data has been freeware since 2004.
- Based on **[OpenTyrian](https://github.com/opentyrian/opentyrian)** by The OpenTyrian Development Team, through its Tyrian 2000 fork OpenTyrian2000.
- **Enhanced Edition** by Patm4nzzz. Independent community project, not affiliated with or endorsed by Eclipse Software or Epic Games.

The code is GPL-2.0-or-later — see [COPYING](COPYING). The Tyrian 2000 data and the art, music and sounds derived from it are separate freeware assets, not covered by the GPL; see `DATA-NOTICES.txt` in the download and [docs/asset-rights.md](docs/asset-rights.md).

Website: **[tyrian-enhanced.com](https://tyrian-enhanced.com/)** · Wiki: **[tyrian-enhanced.com/wiki](https://tyrian-enhanced.com/wiki/)** · Feedback: [contact form](https://tyrian-enhanced.com/contact/) or [GitHub issues](https://github.com/Patm4nzzz/opentyrian-enhanced/issues).
