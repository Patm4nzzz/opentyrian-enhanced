| `data/mod.json`, `data/enemies.json`, `anomalies.json` (global files of the C version, removed) | not read |# Modding Guide — OpenTyrian2000 Enhanced (Godot + Rust)

This guide describes the only mod format read by the Godot + Rust version of the game
(`rewrite/`). Decisions behind it: ADR 0080 to 0083 in `docs/adr/`.

> **C version.** The retired C edition (git tag `c-final`) read a different, older format.
> The Godot + Rust version refuses that format: a C-version mod is listed as an error until it
> is converted ([§11](#11-converting-a-c-version-mod)).

One rule runs through the whole format: **everything a mod uses is named**. The weapons, ships,
enemies and sprites a mod creates are named by their author; the original game content a mod
copies or changes is cited by its name too. The engine assigns every slot in the game tables
itself; a mod author never chooses a number.

---

## Table of contents

1. [Overview](#1-overview)
2. [Quick start](#2-quick-start)
3. [Installing, enabling and checking a mod](#3-installing-enabling-and-checking-a-mod)
4. [`mod.json`](#4-modjson)
5. [Player items: weapons, ships, sidekicks, shields, specials](#5-player-items-weapons-ships-sidekicks-shields-specials)
6. [Sprites](#6-sprites)
7. [Enemies and behaviors](#7-enemies-and-behaviors)
8. [Levels](#8-levels)
9. [Untrusted files: limits](#9-untrusted-files-limits)
10. [Troubleshooting](#10-troubleshooting)
11. [Converting a C-version mod](#11-converting-a-c-version-mod)
12. [Creation kit](#12-creation-kit)

---

## 1. Overview

A mod is a folder:

```
my_mod/
  mod.json              required: "format_version": 1 and "name"
  README.md             optional: shown in Setup → Mods (1 KiB at most)
  levels/ep1_09.json    optional: events of a level (§8)
  sprites/shots/*.png   optional: images, cited by their file name (§6)
```

What a mod can do:

| Content | Section |
|---|---|
| New weapons, ships, sidekicks, shields, specials — from scratch or copied from an original item | §5 |
| Shop availability and the starting loadout | §5.9, §5.10 |
| Replace any named original sprite, add new sprites | §6 |
| New enemies with data-driven behavior (path, tracking, turrets), enemy weapons; change original enemies | §7 |
| Add or replace the events of an original level (enemy waves) | §8 |

What a mod cannot do yet: change a level's map (tiles), music, sound effects, the palette.

## 2. Quick start

1. Copy `rewrite/mod_template/` into the `mods/` folder of your personal files (Godot user
   folder, see `docs/PLAYER-GUIDE.md` §6), for example `…/mods/my_mod/`.
2. Edit `mod.json`: change `name`, uncomment the weapon example.
3. Check it without starting the game (same messages as the game; exit code 1 on any error):
   ```
   cd rewrite
   cargo run --release -p tyrian-mods -- check --data ../data --mods <folder holding my_mod> --enable-all
   ```
4. Start the game, open **Setup → Mods**, enable the mod with **Enter**, leave with **Esc**
   (the mods are reloaded at once).

## 3. Installing, enabling and checking a mod

**Where mods are read**, in this order: `userdata/mods/` next to the game executable (the player's mods and those exported by
the level editor), then `rewrite/mods/` (mods shipped with the game). For two folders with the
same name, the first one wins (warning). Mods are loaded in alphabetical folder order, at most
32.

**Installing from the game**: **Setup → Mods** ends with two lines.

- **Open mods folder** opens `userdata/mods/` in the system file manager (the folder is created if
  missing; its full path is shown under the line). Copy, update or delete mod folders there.
- **Install a mod (.zip)** opens a file dialog (the system one when available); a `.zip` can also
  be dropped on the game window while the Mods menu is shown. The archive is extracted into
  `userdata/mods/<name>/`, then mods are reloaded and the new mod appears in the list (OFF until you
  enable it, unless its `mod.json` says `"enabled": true`).

The archive must hold `mod.json` either at its root (the folder is then named after the `.zip`,
`my_mod.zip` → `my_mod/`) or inside a single top-level folder (that folder's name is kept; a
`__MACOSX/` folder is ignored). Refused, with the reason on the install line and nothing written:
an unreadable archive, an entry that would land outside the mod folder (`..`, absolute path,
Windows drive `C:`, backslash), no `mod.json` as above, and a mod folder that already exists.
There is no silent replacement: to update a mod, delete its folder through **Open mods folder**,
then install the new archive. Installing does not check the mod's content: a broken mod is
installed and then shows as ERR with its reason, like any other.

**Enabling**: **Setup → Mods**. Each mod is marked ON, OFF or ERR; an ERR line shows the reason.
**Enter** toggles the selected mod, **Esc** applies (immediate reload) and returns. The choice is
kept in `userdata/mods.cfg`, never written into `mod.json` (`"enabled"` there is only the initial
state). Launch options, not remembered: `-- --mods=off` disables every mod; `-- --mods=a,b`
enables only `a` and `b`.

**Checking without the game** (the `tyrian-mods` tool, `rewrite/crates/tyrian-mods`):

```
cd rewrite
cargo run --release -p tyrian-mods -- check --data ../data [--mods DIR] [--enable-all] [--only FOLDER]
cargo run --release -p tyrian-mods -- run   --data ../data --level ep1_09 --ticks 700 --enable-all
```

`--mods` defaults to `mods`, i.e. `rewrite/mods` from `rewrite/`. `check` prints, per mod, what
was loaded and every diagnostic; `run` plays a level headless with the mods and counts enemy
appearances (proof that a new enemy really spawns).

**Checking without Rust**: the game shows the reason of an ERR mod in **Setup → Mods**, and
[`rewrite/mods/mod.schema.json`](rewrite/mods/mod.schema.json) is a JSON schema of `mod.json`
(draft-07) that refuses the same unknown keys as the game. Point an editor at it with the
`"$schema"` key (VS Code completes and underlines fields), or check from a shell with
[check-jsonschema](https://github.com/python-jsonschema/check-jsonschema):

```
pip install check-jsonschema json5
check-jsonschema --force-filetype json5 --schemafile rewrite/mods/mod.schema.json my_mod/mod.json
```

The schema checks the structure (keys, types, ranges); names (sprites, items, original enemies)
are checked only by the game and `tyrian-mods check`.

## 4. `mod.json`

JSON with `//` and `/* */` comments and trailing commas allowed. **Every unknown key is an
error**, at every level (top, items, power levels, projectiles, behaviors, starter, levels): a
typo or a leftover of an old format is reported, never silently ignored.

```json5
{
  "$schema": "https://raw.githubusercontent.com/Patm4nzzz/opentyrian-enhanced/rewrite/godot-rust/rewrite/mods/mod.schema.json",  // optional, for editors
  "format_version": 1,          // required: the only version that exists
  "name": "My Mod",             // required
  "author": "Me", "version": "1.0", "description": "…",
  "enabled": false,             // initial state; Setup → Mods overrides it
  "tint": "green",              // hue of every PNG of the mod (§6.3)
  "weapons":   [ … ],  "ships":    [ … ],  "sidekicks": [ … ],
  "shields":   [ … ],  "specials": [ … ],
  "enemy_weapons": [ … ],  "enemies": [ … ],
  "starter": { … }
}
```

| Key | Type | Meaning |
|---|---|---|
| `$schema` | string | optional; path or URL of `mod.schema.json`, for editors; ignored by the game |
| `format_version` | `1` | required; any other value or its absence is an error |
| `name` | string | required; display name |
| `author`, `version`, `description` | string | shown in Setup → Mods |
| `enabled` | bool | initial state (default `true`) |
| `tint` | string or number | §6.3 |
| `weapons`, `ships`, `sidekicks`, `shields`, `specials` | arrays | §5 |
| `enemy_weapons`, `enemies` | arrays | §7 |
| `starter` | object | §5.10 |

## 5. Player items: weapons, ships, sidekicks, shields, specials

### 5.1 Common fields

| Field | Meaning |
|---|---|
| `name` | required, unique in the mod; it is the item's identity (saves, `starter`) |
| `base` | NAME of an original item to copy (case, spaces, dashes and underscores ignored); every field you set overrides the copy |
| `cost` | shop price |
| `icon_sprite` | NAME of the shop icon (a PNG of the mod or an original sprite) |
| `always_available` | sold in every shop (§5.9) |

The engine assigns a free slot to each item, in folder order: at most 20 weapons, 14 ships,
13 sidekicks, 9 shields and 16 specials, all mods together. An item past that limit is refused
with an error.

The shortest useful item copies an original one:

```json5
{ "name": "Alien Vulcan Cannon", "base": "Vulcan Cannon", "always_available": true }
```

### 5.2 Weapons

| Field | Default without `base` | Meaning |
|---|---|---|
| `power_use` | 20 | energy used per shot |
| `tint` | — | hue of this weapon's sprites only (§6.3) |
| `front[]`, `rear[]` | — | power levels (≤ 11 each, §5.3) |
| `cost` | 1000 | |

A weapon with only `front[]` or only `rear[]` has one firing mode; with both, the player switches
modes with the rear-fire key. With `always_available`, a weapon with `front[]` is sold in the
front-gun shop, one with `rear[]` in the rear-gun shop, one with both (or neither) in both.

### 5.3 Power levels and enemy weapons

Each entry of `front[]` / `rear[]` is one power level. An enemy weapon (§7.1) has the same fields
plus `name`. The engine assigns each power level and each enemy weapon a projectile slot
(1200 at most, all mods together).

| Field | Meaning |
|---|---|
| `fire_delay` | frames between two shots |
| `projectile_count` | projectiles per shot |
| `max_on_screen` | length of the cycle through `projectiles[]`; brought down to the number of drawn projectiles (warning) |
| `animation` | animation speed; 0 = no frame cycling |
| `offset_x`, `offset_y` | spawn offset |
| `homing` | `true` or a number; above 5 = guided towards the nearest enemy |
| `accel_x`, `accel_y` | acceleration per frame |
| `circle_radius` | orbit radius |
| `sound` | sound effect index |
| `trail` | trail effect; 255 = none (default) |
| `ship_blast_filter` | damage type mask |
| `projectiles[]` (≤ 8) | §5.4 |

### 5.4 Projectiles and damage codes

| Field | Default | Meaning |
|---|---|---|
| `damage` | 0 | damage code, below |
| `sub_delay` | 255 | shot lifetime in frames; 255 = until off screen; never 0 (the shot is never drawn) |
| `speed_x`, `speed_y` | 0 | velocity; for a player weapon positive `speed_y` goes UP, for an enemy weapon DOWN |
| `spawn_x`, `spawn_y` | 0 | spawn position offset |
| `sprite` | — | NAME of the projectile sprite (§6) |

`damage` is a number of hit points, or a named special effect:

| Value | Effect |
|---|---|
| `0`..`98` | damage per hit |
| `"freeze"` | no damage, freezes the enemy |
| `{ "pierce": n }` (n = 0..5) | goes through enemies, `n` damage to each |
| `{ "chain": "<original weapon>" }` | chain reaction: 1 damage, the hit fires that original enemy weapon (§7.5) |

Any other number (99 and above) is an error: the engine's internal codes are never written.

### 5.5 Ships

| Field | Default without `base` | Meaning |
|---|---|---|
| `damage_resistance` | 20 | armor |
| `cost` | 5000 | |
| `ship_sprite` | — | NAME of the ship image (24×28 PNG); the engine copies it into the four banking poses |
| `ship_poses` | — | `{ "neutral", "left_full", "left", "right", "right_full" }`, a NAME each, to draw the poses yourself |

Every new ship PNG takes one place in the ship image table shared by all enabled mods. The table holds five poses for each of the 14 mod ship slots (70 PNGs, ADR 0134), so every slot can have its own five poses: `alien_ship_pack` (8 ships, 40 PNGs) and `bio_ships_pack` (6 ships, 30 PNGs) fill it together. A PNG past those 70 places is named by a warning "table ships full".

### 5.6 Sidekicks

| Field | Default without `base` | Meaning |
|---|---|---|
| `trail` | 0 | movement: 0 fires with the player, 1 follows, 2 orbits, 3 independent, 4 stationary orbit |
| `ammo` | 0 | shots before recharge; 0 = unlimited |
| `cost` | 5000 | |

### 5.7 Shields

| Field | Default without `base` | Meaning |
|---|---|---|
| `shield_power` | 9 | recharge rate, 0..28 (clamped, warning) |
| `max_power` | 14 | maximum, 0..18 (clamped, warning; the strongest original shield has 14) |
| `cost` | 10000 | |

### 5.8 Specials

| Field | Default without `base` | Meaning |
|---|---|---|
| `power` | 100 | effect strength |
| `subtype` | 1 | effect kind |

The weapon a special fires is the one of its `base`. Shops never sell specials: give one through
`starter`.

### 5.9 Shop slots

`always_available` items are added to every shop, at most 16 per category, after the level's own
items (at most 10). The list of a category scrolls, five rows at a time; the C version stopped at
10 items in all, which hid the items of a second pack (ADR 0134).

### 5.10 Starting loadout

```json5
"starter": { "ship": "Xeno Vanguard", "front_weapon": "Alien Vulcan Cannon",
             "rear_weapon": "…", "shield": "…", "generator": "…",
             "sidekick_left": "…", "sidekick_right": "…" }
```

Each value is the exact NAME of an item (original or from a loaded mod). Names are resolved after
every mod has loaded; an unknown name is an error.

## 6. Sprites

### 6.1 Categories

PNG files go into `sprites/<category>/<name>.png`:

| Folder | Content |
|---|---|
| `shots/` | projectiles |
| `ships/` | player ships (24×28) |
| `sidekicks/` | sidekicks |
| `pickups/`, `pickups/weapons/`, `pickups/shields/`, `pickups/specials/` | shop icons and pickups |
| `coins/` | coins, gems, data cubes |
| `weapon_icons/` | HUD icons |
| `explosions/`, `portraits/`, `planets/` | explosions, portraits, planets |
| `enemies/<enemy name>.png` | image of an original enemy (§7.4) |

Any RGBA PNG works; it is mapped to the game palette at load time (alpha 0 = transparent).
Templates of the original sprites: `data/reference/sprites/` in the repository (not loaded by the
game); names of the original sprites: `data/sprite_manifest.json`.

### 6.2 Names

A PNG is named either:

- after an **original sprite** (`data/sprite_manifest.json`): it replaces it everywhere. Shop
  icons in `pickups/weapons/` are named after the part after the slash (`multi_cannon.png`
  replaces `weapons/multi_cannon`);
- with a **new name**: it is registered under that name, and cited by it in `sprite`,
  `ship_sprite`, `icon_sprite`, `ship_poses`.

A PNG named by a number is an error.

### 6.3 Tint

`"tint"` at the top of `mod.json` recolors every PNG of the mod; on a weapon, it recolors that
weapon's sprites (copies, the originals stay unchanged). Values: `"red"`, `"orange"`, `"yellow"`,
`"green"`, `"cyan"`, `"blue"`, `"purple"`, `"magenta"`, `"pink"`, or a hue `0.0`–`1.0`.

## 7. Enemies and behaviors

### 7.1 Enemy weapons

```json5
"enemy_weapons": [ { "name": "drone_spread", /* fields of a power level, §5.3 */ } ]
```

### 7.2 `enemies[]`

Two kinds of entries, told apart by their key:

```json5
"enemies": [
  { "name": "my_drone",      // NEW enemy, cited by its name in levels/*.json
    "base": "tech_corridor_32", // original enemy copied (optional, §7.5)
    "armor": 8,              // + any enemy field, §7.3
    "behavior": {
      "path":  [ { "ticks": 30, "vx": 2, "vy": 2 }, { "ticks": 30, "vx": -2, "vy": 2 } ],
      "loop":  true,
      "track": { "axis": "x", "max_speed": 2, "every": 4 },
      "turrets": [ { "weapon": "drone_spread", "every": 40, "start": 20, "direction": "down" } ]
    } },
  { "original": "jungle_fighters_07", "armor": 30 }   // changes an original enemy
]
```

A new enemy without `base` starts from safe values (visible, destructible, armor 10). At most 100
new enemies, all mods together. An `original` entry writes only the fields it sets.

### 7.3 Enemy fields

| Field | Meaning |
|---|---|
| `armor` | hit points (0 makes it a pickup, warning) |
| `value` | score |
| `xmove`, `ymove`, `xaccel`, `yaccel`, `xcaccel`, `ycaccel`, `xrev`, `yrev` | movement |
| `startx`, `starty`, `startxc`, `startyc` | spawn offsets |
| `esize` | hit box size |
| `ani`, `animate`, `egraphic[]` (≤ 20), `shapebank` | images |
| `explosiontype`, `dgr`, `dlevel`, `dani` | death animation |
| `tur[]` (≤ 3), `freq[]` (≤ 3) | original turrets: NAMES of original enemy weapons (`null` = none) and their frequencies; a mod's enemy weapon is fired through `behavior.turrets` |
| `elaunchfreq`, `elaunchtype` | launched enemy (NAME of a new enemy defined above, or of an original enemy) |
| `eenemydie` | enemy left on death (same) |

### 7.4 Behaviors

| Field | Values | Effect |
|---|---|---|
| `path[]` (≤ 32) | `ticks` 1..10000, `vx`/`vy` −128..127 | sets the speeds for `ticks` frames; `loop` starts again |
| `track` | `axis` x/y/xy, `max_speed` 0..127, `every` 1..255 | moves one unit towards the player every `every` frames (no randomness) |
| `turrets[]` (≤ 4) | `weapon` (NAME of an enemy weapon of the mod or of an original enemy weapon), `every`/`start` 1..255, `direction` down/side/side_reverse | fired by the original turret code |

Any error in a behavior refuses the whole behavior. Everything is integer and deterministic; the
original content is not affected (golden masters unchanged).

Worked example: `rewrite/mods/_retired/example_behaviors/` (enemy weapon `drone_spread`, new enemy
`zigzag_drone` with a path, a tracking move and a turret, added to level `ep1_09`, plus the
"Helix Driver" player weapon). It is retired from the release but kept, and tested
(`tyrian-mods` integration tests), as the reference for this section.

### 7.5 Names of the original enemies and enemy weapons

The original enemies and enemy weapons have no names in the game data. The engine computes stable
names from the data, published in [`rewrite/mods/original-names.json`](rewrite/mods/original-names.json)
with their armor, score, size and table slots, and drawn on one preview sheet per image bank in
`docs/original-enemies/`. That folder is not in the repository: it is generated by
`tyrian-mods catalog --sheets` (command below). The names follow these rules:

- an **enemy** is `<theme>_<NN>`: the theme of its image bank (`tech_corridor`, `jungle_fighters`,
  …) and the rank of its first image in that bank (`tech_corridor_32`). Enemies sharing an image
  but with other statistics get `b`, `c`… (`tech_corridor_32b`). An enemy without image is
  `no_image_<NN>`. Episodes 1-3 and 4-5 use two different tables: one name designates the same
  enemy in both when it exists in both (its slot can differ); the JSON file lists the slots;
- an **enemy weapon** is `<sprite>_<NN>`: the name of its first projectile sprite
  (`data/sprite_manifest.json`) and a counter (`ring_green_leftdown_01`).

Regenerate both after a change of the game data (the CI checks that the file is up to date):

```
cd rewrite
cargo run --release -p tyrian-mods -- catalog --data ../data --sheets ../docs/original-enemies
```

An original enemy's image is replaced by `sprites/enemies/<enemy name>.png` (§7.5): the PNG
takes the place of the enemy's first image in its bank, in every episode. A sub-folder (the old
`<bank>/<image>.png` convention) or an unknown name is an error.

## 8. Levels

`levels/epN_NN.json` (N = episode 1..5, NN = level file number):

```json5
{ "episode": 1, "lvl_file_num": 9,
  "add_events": [ { "time": 300, "spawn": "my_drone", "x": 130 },
                  { "time": 300, "spawn": "tech_corridor_32", "x": "random", "link": 3 } ] }
```

| Key | Meaning |
|---|---|
| `events` | replaces the original event list |
| `add_events` | inserted into the original list, each after the original events of the same `time` |
| `random_enemies` (≤ 40) | replaces the level's random enemies (enemy NAMES) |

Every event has a `time` (scroll position 0..65535, the `cur_loc` shown by the level editor) and
exactly one **type key**. The type key holds the main parameter (`"spawn": "my_drone"`), or `true`
when the type has none (`"stars_off": true`); the other parameters are named and default to 0.
Enemies, image banks and specials are written by NAME (§7.5); special values have names
(`"x": "random"`, `"keep"`, `{ "picked": 2 }` for a link chosen by `pick_random_link`) and their
numeric codes are refused. An unknown type or parameter is an error naming the event.
The map (tiles) cannot be changed. The engine behavior of each type: `docs/events-spec.md`.

The level editor's **Export to the mod** button writes this format (the same table is used by the
reader, so every original level exported and read back plays identically).

| Type key | Main value | Other parameters |
|---|---|---|
| `nothing` | `true` |  |
| `starfield_speed` | `speed` |  |
| `scroll_speed` | `back` | `back2`, `back3` |
| `scroll_slow` | `true` |  |
| `stop_map` | `layers` |  |
| `load_banks` | `bank1` (bank NAME or null) | `bank2` (bank NAME or null), `bank3` (bank NAME or null), `bank4` (bank NAME or null) |
| `spawn_ground` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `spawn_top` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `stars_off` | `true` |  |
| `stars_on` | `true` |  |
| `spawn_ground2` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `end_level` | `immediate` |  |
| `spawn_2x2` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `layer` ("ground", "sky", "top", "ground2") |
| `random_enemies_off` | `true` |  |
| `random_enemies_on` | `true` |  |
| `spawn` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `text` | `message` |  |
| `spawn_ground_bottom` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `spawn_bottom` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `move_enemies` | `x_speed` ("keep") | `y_speed` ("keep"), `targets` ("ground", "sky", "top", "all", { "picked": n }), `link`, `cycle`, `fixed_move_y` ("zero") |
| `accelerate_enemies` | `x` ("keep") | `y` ("keep"), `targets` ({ "picked": n }), `link`, `cycle`, `animation` |
| `background3_over` | `true` |  |
| `background3_under` | `true` |  |
| `spawn_top_bottom` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `animate_enemies` | `animation` | `cycle`, `mode`, `link` |
| `set_armor` | `armor` | `link` |
| `small_enemy_adjust` | `on` |  |
| `reverse_enemies` | `x` ("keep") | `y` ("keep"), `color` ({ "picked": n }), `link` |
| `top_enemies_under` | `true` |  |
| `top_enemies_over` | `true` |  |
| `scroll_speed_keep_stop` | `back` | `back2`, `back3` |
| `fire_rate` | `rate1` | `rate2`, `rate3`, `link` ("all"), `launch_rate` |
| `spawn_top_low` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `drop_on_death` | `enemy` (enemy NAME) | `link` |
| `music_fade` | `true` |  |
| `music` | `song` |  |
| `end_level_when_clear` | `true` |  |
| `random_enemy_rate` | `rate` |  |
| `jump_to` | `time` |  |
| `relink` | `from` | `to` |
| `continual_damage` | `true` |  |
| `clear_enemies` | `sky_only` |  |
| `background3_mode2` | `true` |  |
| `background2_over` | `mode` |  |
| `color_filter` | `mode` | `color`, `brightness`, `new_color`, `brightness_change`, `fade_start` |
| `drop_on_death_multiplayer` | `enemy` (enemy NAME) | `link` |
| `difficulty` | `change` | `condition`, `damage_rate` |
| `set_armor_plain` | `armor` | `link` |
| `background2_opaque` | `true` |  |
| `custom_enemy_ground` | `image` | `x` ("random", "none"), `bank` (bank NAME or null), `link`, `y`, `armor` |
| `custom_enemy` | `image` | `x` ("random", "none"), `bank` (bank NAME or null), `link`, `y`, `armor` |
| `custom_enemy_top` | `image` | `x` ("random", "none"), `bank` (bank NAME or null), `link`, `y`, `armor` |
| `custom_enemy_ground2` | `image` | `x` ("random", "none"), `bank` (bank NAME or null), `link`, `y`, `armor` |
| `force_events` | `mode` ("off") |  |
| `jump` | `time` ("return") |  |
| `set_acceleration` | `x` ("keep") | `y` ("keep"), `targets` ({ "picked": n }), `link` |
| `spawn_ground2_low` | `enemy` (enemy NAME) | `x` ("random", "none"), `y_speed`, `link`, `y`, `fixed_move_y` |
| `jump_on_boss_death` | `time` |  |
| `set_launch` | `enemy` (enemy NAME) | `link` ("all") |
| `replace_enemies` | `enemy` (enemy NAME) | `link` |
| `special_enemy` | `flag` | `set`, `link` |
| `skip_if_flag` | `flag` | `value`, `skip` |
| `sound` | `sound` |  |
| `skip_if_one_player` | `skip` |  |
| `smoothie` | `effect` | `on`, `data` |
| `background3_1x1` | `mode` |  |
| `skip_by_difficulty` | `difficulty` | `skip` |
| `level_timer` | `on` | `target`, `seconds` |
| `invulnerable` | `ticks` |  |
| `jump_if_cleared` | `time` | `link1`, `link2`, `link3` |
| `jump_if_map_before` | `time` | `position` |
| `background3_variant_b` | `on` |  |
| `sky_enemies_over_all` | `on` |  |
| `bounce` | `x_max` ("keep") | `y_max` ("keep"), `link`, `x_min` ("keep"), `y_min` ("keep") |
| `pick_random_link` | `from` | `to`, `slot` (0..9), `skip` |
| `return_when_clear` | `true` |  |
| `set_map_position` | `y` | `y2` |
| `galaga_fire_rate` | `true` |  |
| `boss_bar` | `link` | `link2` |
| `skip_if_two_players` | `skip` |  |
| `wrap_background2` | `from` | `to` |
| `give_special` | `special` (special NAME) |  |
| `timed_battle_timer` | `on` | `target`, `seconds` |
| `timed_battle_drop` | `enemy` (enemy NAME) | `link` |
| `random_explosions` | `on` |  |

## 9. Untrusted files: limits

A mod is untrusted data. A file over a limit produces an error or a warning, never a crash:
JSON ≤ 1 MiB, depth ≤ 48, string ≤ 16 KiB, ≤ 200 000 values; ≤ 32 mods; PNG ≤ 4 MiB,
≤ 2048 px per side, ≤ 1024 per folder; ≤ 2500 events per level, ≤ 40 random enemies, ≤ 64 level
files per mod; README ≤ 1 KiB.

## 10. Troubleshooting

| Symptom | Check |
|---|---|
| The mod is ERR in Setup → Mods | read the reason on its line, then run `tyrian-mods check` (§3) for every message |
| "C-version mod" error | the `mod.json` has no `"format_version": 1`: convert it (§11) |
| "unknown key" | a typo, or a field of the C-version format (§11) |
| A weapon makes a sound but nothing is drawn | the first projectile's `sprite` is missing or unknown (warning in `check`) |
| An item is not in the shop | add `"always_available": true`; at most 16 such items per shop category, all mods together (§5.9) |
| A new enemy never appears | cite it by its `name` in `levels/*.json`; `tyrian-mods run --level epN_NN` counts its appearances |
| A ship is invisible | `ship_sprite` must name a PNG of `sprites/ships/` (24×28) or an original ship sprite |

## 11. Converting a C-version mod

What changes, field by field:

| C version | This format |
|---|---|
| (absent) | `"format_version": 1` |
| `"format": 2`, `"$schema"` of the C schema | removed |
| `"new_enemies": [ { "name": … } ]` | `"enemies": [ { "name": … } ]` |
| `"enemies": [ { "id": 125, … } ]` | `"enemies": [ { "original": "<name>", … } ]` (§7.5) |
| `"damage": 99` / `100+n` / `250+n` | `"freeze"` / `{ "chain": "<name>" }` / `{ "pierce": n }` |
| level events `{ "eventtime": 300, "eventtype": 15, "eventdat": 10, "eventdat2": 130 }` | `{ "time": 300, "spawn": "<enemy name>", "x": 130 }` (§8) |
| `"behavior": { "version": 1, … }` | `"behavior": { … }` |
| `"_weapon_id"`, `"level"` in a power level | removed (assigned by the engine) |
| `"sprite": 12` or `"sprite_12"` | `"sprite": "<manifest name>"` |
| `"starter": { "ship": 19 }` | `"starter": { "ship": "<name>" }` |
| ship `"speed"`, `"power_drain"`, `"bigshipgraphic"`, special `"weapon_id"` | removed (no effect, or assigned by the engine) |
| aliases `fire`, `count`, `max`, `dmg`, `life`, `speed` | full names only |
| `levels/*.json`: `"format": 1` | removed |
| `sprites/<cat>/<number>.png` | renamed after the replaced sprite |
| `sprites/enemies/<bank>/<image>.png` | `sprites/enemies/<enemy name>.png` (§7.4) |
| `data/mod.json`, `data/enemies.json`, `anomalies.json` (global files of the C version) | not read |

Converted examples, all three now in `rewrite/mods/_retired/` (kept for reference, not shipped):
`alien_pack`, `example_nova_weapons` and `alien_ships` (C originals no longer in the repository). The conversion was
checked: same tables, sprites, shop and starting loadout as the former loader, on all five
episodes.

## 12. Creation kit

- Template to copy: `rewrite/mod_template/`.
- Shipped packs: `rewrite/mods/` (see its `README.md`): ships with five drawn poses
  (`alien_ship_pack`, `bio_ships_pack`), weapons with eleven power levels and their own shot
  PNGs (`alien_weapon_pack`, `bio_weapons_pack`, `elements_weapon_pack`).
- Commented examples, retired from the release but kept: `rewrite/mods/_retired/` —
  `example_nova_weapons` (weapons from scratch, piercing and homing shots), `alien_pack`
  (`base`, `tint`, `starter`), `example_behaviors` (enemy weapon, new enemy, behavior, level
  events). Check them with `cargo run -p tyrian-mods -- check --mods mods/_retired --enable-all`
  or copy one into your mods folder to play it.
- Godot level editor, **Tyrian** dock: Add an enemy here → Export to the mod → Build the .zip
  package → Play from here.
- Step-by-step tutorial with screenshots: [`docs/FIRST_LEVEL_15_MIN.md`](docs/FIRST_LEVEL_15_MIN.md).
