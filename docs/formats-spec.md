# Tyrian 2000 data format specification (as read by OpenTyrian2000 Enhanced)

> Purpose: to allow writing a **byte-exact** (Rust) reader/writer for the
> files in `data/`. Each field cites the C function that reads it
> (`file.c:function`). All sizes and counts below were
> **verified against the files present in `data/`** (ad hoc Python script,
> October 2026) — see § 14 for the summary table.

### C reference used
Written on the `rewrite/godot-rust` branch (4ca88d28), then **checked against
`wip/sept-2026` (439eee2e, the most recent C reference)**: in the 14
readers cited (`episodes.c, level_load.c, lvllib.c, sprite.c, palette.c,
picload.c, nortsong.c, loudness.c, lds_play.c, helptext.c, editship.c,
mainint.c, game_menu.c, animlib.c`), the sequence of `fread_*`/`fseek`/`dir_fopen*`
calls is **identical** between the two revisions
(the differences concern comments, Sprite2 display bounds and
mods). No binary file in `data/` (`*.lvl, *.dat, *.shp, *.hdt, *.snd,
*.mus, *.pic`) differs. The specification therefore holds for both.
The only notable behavior change in wip: the Sprite2 index guard
becomes `index > sprite2_count()` (1-based indices) instead of `index*2 > size`.

## 0. Common conventions

### 0.1 File location
`src/file.c:data_dir()` picks the first directory containing `tyrian1.lvl`
among: `--data` (custom_data_dir), `TYRIAN_DIR` (compile-time), `data`, `.`,
`<exe>/data`, `<exe>`. All files are then opened with
`dir_fopen(data_dir(), name, "rb")` (`dir_fopen_die` = the game stops if the file is missing).
Names are lowercase (`tolower` in `sprite.c:JE_loadCompShapes` and
`level_load.c:JE_loadMap`); beware of case on case-sensitive systems.

### 0.2 Read primitives (`src/file.h`)

| Primitive | Size | Note |
|---|---|---|
| `fread_u8_die` / `fread_s8_die` | 1 | |
| `fread_bool_die` | 1 | any value ≠ 0 → `true` (information loss: write the original byte for the round-trip) |
| `fread_u16_die` / `fread_s16_die` | 2 | **little-endian** (swapped only on a big-endian host) |
| `fread_u32_die` / `fread_s32_die` | 4 | **little-endian** |

**All multi-byte values are little-endian**, with two exceptions:
- the level `mapSh` table (§ 1.3): **big-endian** `u16` (read LE then unconditional `SDL_Swap16`);
- the `demo.N` delays (§ 9): **big-endian** `u16` (assembled by hand `b0<<8 | b1`).

### 0.3 Encrypted Pascal string (`helptext.c:read_encrypted_pascal_string` / `decrypt_string`)
```
u8   len
u8   data[len]       ; encrypted
```
Key: `K = [204, 129, 63, 255, 71, 19, 25, 62, 1, 99]` (10 bytes).
Decryption (from end to start, over the **encrypted** bytes c[]):
```
p[i] = c[i] ^ K[i % 10] ^ (i > 0 ? c[i-1] : 0)
```
Encryption (the inverse, from start to end):
```
c[i] = p[i] ^ K[i % 10] ^ (i > 0 ? c[i-1] : 0)
```
No terminator; `len` may be 0 (empty string — `levels1.dat` contains 66 of them).
Decrypt → re-encrypt round-trip **verified byte-exact** on the 5
`levelsN.dat`, the 5 `cubetxtN.dat` and `tyrian.cdt`. The same algorithm is
duplicated in `endless.c:endless_decrypt`.
The C read is truncated to `size-1` (assert `len < size`): a writer must keep `len ≤ 255`.

### 0.4 "Offset-table container" files
Recurring pattern (`.lvl`, `.pic`, main `.shp`, `.snd`, `music.mus`):
```
u16  count
s32/u32 offset[count]   ; ABSOLUTE offsets from the start of the file
...  blobs
```
The size of entry `i` = `offset[i+1] - offset[i]`; the last one stops at
the end of the file (`ftell_eof`). There is **no** size field: a writer
must recompute the offsets; in all shipped files `offset[0] = 2 + 4*count`
(no padding).

---

## 1. Levels: `tyrianN.lvl` (map + events) — PRIORITY 1

Read by `lvllib.c:JE_analyzeLevel` (table) then `level_load.c:JE_loadMap`
(contents). Name: `episodes.c:JE_initEpisode` → `"tyrian%hhu.lvl"`.

### 1.1 Header (`lvllib.c:JE_analyzeLevel`)
```
u16  lvlNum                 ; number of entries (capped at 42 by the code)
s32  lvlPos[lvlNum]         ; absolute offsets
```
`lvlPos[lvlNum] = file size` (sentinel added in memory).

Verified layout: `lvlNum = 2 × nbLevels + 1`.
- entry `2k`   : header of level k+1 + enemies + events (§ 1.2);
- entry `2k+1` : `mapSh` + 3 map layers, **fixed size 22,368 bytes** (§ 1.3);
- entry `lvlNum-1` (last): empty (size 0) for episodes 1–3;
  **episode object tables** (HDT format § 2.2, 273,895 bytes including the 77 B tail) for episodes
  4 and 5 (`episodes.c:JE_loadItemDat`: `fseek(lvlPos[lvlNum-1])`).

| File | Size | lvlNum | Levels | Last entry |
|---|---|---|---|---|
| tyrian1.lvl | 538 856 | 37 | 18 | 0 B |
| tyrian2.lvl | 381 187 | 25 | 12 | 0 B |
| tyrian3.lvl | 393 938 | 25 | 12 | 0 B |
| tyrian4.lvl | 937 303 | 41 | 20 | 273 895 B (episode 4 items) |
| tyrian5.lvl | 521 137 | 17 | 8  | 273 895 B (episode 5 items) |

Level `lvlFileNum` (1-based, read from `]L[` in `levelsN.dat`, § 3) is at
`lvlPos[(lvlFileNum-1)*2]` (`JE_loadMap`). Sections 1.2 and 1.3 are read
**in one go** (the odd entry immediately follows the even entry).

### 1.2 Even entry: header, enemies, events (`level_load.c:JE_loadMap`)

| Off. | Type | Field | Notes |
|---|---|---|---|
| 0 | char | `char_mapFile` | map letter (read, not used afterwards) |
| 1 | char | `char_shapeFile` | → `shapes%c.dat` (lowercase): `W X Y Z )` observed |
| 2 | u16 | `mapX` | initial X position, layer 1 (always 1 in the data) |
| 4 | u16 | `mapX2` | layer 2 (1) |
| 6 | u16 | `mapX3` | layer 3 (1) |
| 8 | u16 | `levelEnemyMax` | capped at 40 (`levelEnemy[40]`, tyrian2.c) |
| 10 | u16[levelEnemyMax] | `levelEnemy` | random enemy ids |
| … | u16 | `maxEvent` | capped at `EVENT_MAXIMUM` = 2500 (lvlmast.h) |
| … | 11 B × maxEvent | `eventRec[]` | see below |

Event record — **11 bytes, read order ≠ struct order**:

| Off. | Type | Field (`varz.h:JE_EventRecType`) |
|---|---|---|
| 0 | u16 | `eventtime` (scroll line that triggers it) |
| 2 | u8  | `eventtype` |
| 3 | s16 | `eventdat` |
| 5 | s16 | `eventdat2` |
| 7 | s8  | `eventdat3` |
| 8 | s8  | `eventdat5` ← note: 5 before 6 before 4 |
| 9 | s8  | `eventdat6` |
| 10 | u8 | `eventdat4` |

After reading, the code appends a sentinel `eventtime = 65500` (not stored).
Largest `maxEvent` observed: 2,259 (tyrian3 level 6).

Event types handled by `level_events.c` (switch on `eventtype`):
1–3 (scrolling/speeds), 4 and 83 (map stop), 5 (loading of enemy
shape banks), 6/7/10/12/15/17/18/23/56… (creation of ground/sky/top/bottom enemies,
4×4), 19/20/24/25/27/47/55/74 (global changes: move, accel, animate, damage,
accelrev, bounce), 31 (forced shot), 32 (create enemy), 33/45/85 (enemy from
enemy), 34/35 (music fade / new track), 39 (linknum), 40 (continuous
damage), 46 (difficulty), 48 (opaque background 2), 58 (launch), 59 (replacement),
60 (special enemy), 61/63/64/66/80 (conditional jumps on flag / 2 players /
difficulty), 62 (sound), 68, 81 (WRAP2), 82 (special weapon), 84 (Timed
Battle timer), 99 (random T2000 explosions). Full list: `grep "case" src/level_events.c`.
For the round-trip, treat the record as 11 typed opaque bytes.

### 1.3 Odd entry: shapes + maps (22,368 bytes)

| Off. | Size | Contents |
|---|---|---|
| 0 | 3 × 128 × u16 **BE** = 768 | `mapSh[3][128]`: for each layer, tile index (1-based) in `shapesX.dat`; 0 = none |
| 768 | 14 × 300 = 4 200 | layer 1 (`megaData1.mainmap[300][14]`), 1 byte = index into `mapSh[0]` |
| 4 968 | 14 × 600 = 8 400 | layer 2 (`[600][14]`) → `mapSh[1]` |
| 13 368 | 15 × 600 = 9 000 | layer 3 (`[600][15]`) → `mapSh[2]` |

Row-major order (`y` then `x`). Only indices 0..71 of `mapSh[n]` are
consulted (72 shapes max for layer 1, 71 for layer 2 with x=71 forced NULL, 70 for layer 3
with x≥70 forced NULL — `JE_loadMap`, structs `JE_MegaDataType1..3` in `varz.h`);
entries 72..127 must be kept as-is.

### 1.4 Tiles: `shapesX.dat` (`level_load.c:JE_loadMap`)
600 sequential records:
```
u8   blank                     ; ≠0 → empty tile, nothing follows
u8   pixels[24*28] (if blank=0) ; 672 B, 8 bpp palette-indexed, 24 wide × 28 high
```
**520 unread tail bytes** in all 5 files (looks like `u16` values
— purpose unknown): keep verbatim.

| File | Size | Empty | Tail |
|---|---|---|---|
| shapesw.dat | 172 480 | 345 | 520 |
| shapesx.dat | 217 504 | 278 | 520 |
| shapesy.dat | 206 752 | 294 | 520 |
| shapesz.dat | 318 976 | 127 | 520 |
| shapes).dat | 118 720 | 425 | 520 |

Color 0 in the tile = transparent (`fill` computation of layers 2/3).

---

## 2. `tyrian.hdt` — object, weapon and enemy tables — PRIORITY 2

Verified size: **295,069 B**.

### 2.1 Overall structure

| Off. | Contents | Reader |
|---|---|---|
| 0 | `s32 episode1DataLoc` = **21 174** | `episodes.c:JE_loadItemDat`, `helptext.c:JE_loadHelpText` |
| 4 .. 21 173 | 677 encrypted Pascal strings (§ 0.3): original help/menu texts ("** Section 1 - Online Help …") | **no longer read** since Phase 2C (replaced by `data/locales/*.txt`) — keep verbatim |
| 21 174 .. end | object block (§ 2.2), 273,895 B on disk: 273,818 B read by `JE_loadItemDat` + 77 B unread tail | `JE_loadItemDat` |

For episodes 4 and 5, the same block (different data, same size
273,895 B, of which 273,818 B are read) is read from the last entry of `tyrian4.lvl` / `tyrian5.lvl`.

### 2.2 Object block (`episodes.c:JE_loadItemDat`)
Names are Pascal `string[30]`: `u8 len` + **30 fixed bytes**
(space-padded in the data; `trim_name` strips them in memory —
keep the 30 raw bytes for the round-trip).

| Order | Section | Record count | Rec. size | Abs. offset (hdt) |
|---|---|---|---|---|
| 1 | `u16 itemNum[7]` = (818, 60, 6, 18, 37, 11, 850) — informational, **not used** for reading | 1 | 14 | 21 174 |
| 2 | weapons `weapons[0..818]` then `[1000..1818]` | 819 + 819 = 1 638 | 80 | 21 188 |
| 3 | weapon ports `weaponPort[0..60]` | 61 | 82 | 152 228 |
| 4 | specials `special[0..54]` | 55 | 37 | 157 230 |
| 5 | generators `powerSys[0..6]` | 7 | 37 | 159 265 |
| 6 | ships `ships[0..18]` | 19 | 41 | 159 524 |
| 7 | options/sidekicks `options[0..37]` | 38 | 86 | 160 303 |
| 8 | shields `shields[0..11]` | 12 | 37 | 163 571 |
| 9 | enemies `enemyDat[0..850]` then `[1001..1850]` | 851 + 850 = 1 701 | 77 | 164 015 |
| — | **77 unread tail bytes** (exactly 1 additional enemy record) | 1 | 77 | 294 992 |

The counts are **fixed by the format** (constants `hdt_*_count`,
`WEAP_END1=818`, `WEAP_START2=1000`, `HDT_WEAP_BANK2_END=1818`,
`ENEMY_END1=850`, `ENEMY_START2=1001`, `ENEMY_NUM=1850` in `lvlmast.h`),
not by `itemNum`. The in-memory arrays are larger (JSON mods).

**Weapon (80 B)**:
`u16 drain; u8 shotrepeat; u8 multi; u16 weapani; u8 max; u8 tx; u8 ty; u8 aim;
u8 attack[8]; u8 del[8]; s8 sx[8]; s8 sy[8]; s8 bx[8]; s8 by[8]; u16 sg[8];
s8 acceleration; s8 accelerationx; u8 circlesize; u8 sound; u8 trail; u8 shipblastfilter`

**Weapon port (82 B)**: `u8 nameLen; char name[30]; u8 opnum; u16 op[0][11]; u16 op[1][11]; u16 cost; u16 itemgraphic; u16 poweruse`

**Special (37 B)**: `u8 nameLen; char name[30]; u16 itemgraphic; u8 pwr; u8 stype; u16 wpn`

**Generator (37 B)**: `u8 nameLen; char name[30]; u16 itemgraphic; u8 power; s8 speed; u16 cost`

**Ship (41 B)**: `u8 nameLen; char name[30]; u16 shipgraphic; u16 itemgraphic; u8 ani; s8 spd; u8 dmg; u16 cost; u8 bigshipgraphic`

**Option (86 B)**: `u8 nameLen; char name[30]; u8 pwr; u16 itemgraphic; u16 cost; u8 tr; u8 option; s8 opspd; u8 ani; u16 gr[20]; u8 wport; u16 wpnum; u8 ammo; u8 stop(bool); u8 icongr`

**Shield (37 B)**: `u8 nameLen; char name[30]; u8 tpwr; u8 mpwr; u16 itemgraphic; u16 cost`

**Enemy (77 B)**:
`u8 ani; u8 tur[3]; u8 freq[3]; s8 xmove; s8 ymove; s8 xaccel; s8 yaccel; s8 xcaccel; s8 ycaccel;
s16 startx; s16 starty; s8 startxc; s8 startyc; u8 armor; u8 esize; u16 egraphic[20];
u8 explosiontype; u8 animate; u8 shapebank; s8 xrev; s8 yrev; u16 dgr; s8 dlevel; s8 dani;
u8 elaunchfreq; u16 elaunchtype; s16 value; u16 eenemydie`

Consistency checks verified: port 0 = "None", port 1 = "Pulse-Cannon",
ship 1 = "USP Talon Light Fighter", shield 11 = "MicroCorp HXS Class C",
generator 6 = "Gravitron Pulse-Wave". After reading, `hdt_json_try_override`,
`hdt_json_load_mod`, `mod_manager_load` may overwrite the values (JSON
`data/mod.json`, `data/enemies.json`, `data/mods/`) — outside the binary format.
`enemyDat[].value` > 30000 encodes a power-up (bands 30000/31000/32000/32100,
`episodes.c:validate_powerup_values`).

`data/doc/tyrian.hdt.txt` documents the same file (external reference).

---

## 3. Episode script: `levelsN.dat` — PRIORITY 1

Read by `level_load.c:JE_loadMap` (and `endless.c:build_level_pool`). File =
a contiguous sequence of encrypted Pascal strings (§ 0.3), **with no header or table**.
Round-trip verified.

| File | Size | Strings | Sections (`*`) |
|---|---|---|---|
| levels1.dat | 9 600 | 481 | 48 |
| levels2.dat | 5 534 | 277 | 24 |
| levels3.dat | 6 052 | 311 | 25 |
| levels4.dat | 11 116 | 683 | 51 |
| levels5.dat | 4 257 | 184 | 20 |

Syntax (decrypted strings, 0-based indices within the string):
- Line starting with `*`: start of a section ("`**1 SECTION - LEVEL #1***…`").
  `mainLevel = n` → skip `n` `*` lines.
- `]X…` directives (switch on `s[1]`) — argument positions read with `atoi`:
  - `]L[`: `nextLevel = atoi(s+9)`, `levelName = s[13..21]` (9 chars),
    `levelSong = atoi(s+22)`, `lvlFileNum = atoi(s+25)`, bonus if `s[27]=='$'`
    (normal bonus) / `s[28]=='$'` (bonus). E.g.: `]L[ 9999 048 SAVARA   21 20`.
  - `]G[`: `mapOrigin = atoi(s+4)`, `mapPNum = atoi(s+7)`, then for i:
    `mapPlanet[i] = atoi(s+1+(i+1)*8)`, `mapSection[i] = atoi(s+4+(i+1)*8)` (max 5).
  - `]?[`: count = `atoi(s+4)`, `cubeList[i] = atoi(s+3+(i+1)*4)` (max 4).
  - `]![` (cubeMax = `atoi(s+4)`), `]+[` (cubeMax += …).
  - `]I[`: followed by **9 strings** of shop categories; list of integers after column 8.
  - `]J`, `]2`, `]w`, `]t`, `]l`: section jump `atoi(s+3)` (various conditions).
  - `]H`: jump `atoi(s+4)` if difficulty < Hard; `]h`: skips the next line if difficulty > Normal.
  - `]W[y|n] NN`: alert text; lines up to a `#` line.
  - `]Q[`: end of episode; 9 blocks of indices terminated by `#`.
  - `]P NNN` (image `tyrian.pic` or palette if > 900; 0 → `tshp2.pcx`), `]U`, `]V`, `]R` (image transitions), `]C`, `]B`, `]F` (fades), `]M NN` (music), `]i NN` (shop music), `]T[` (Timed Battle: `atoi(s + sel*3)`), `]q`, `]A` (`tyrend.anm` animation), `]g` (Galaga), `]x`, `]e` (ENGAGE), `]s`, `]b`, `]@`, `]n`, `]S`.
- Usage counts (`level_load.c` comment): L=76, ?=67, G=67, I=66, P=38, W=34, …; V/R/B/S = 0.

For a byte-exact writer: keep the strings as raw bytes (including
fixed-column spaces and empty strings).

---

## 4. Sprite sheets

### 4.1 "Simple" sprite table (`sprite.c:load_sprites`)
Used by the first 7 tables of `tyrian.shp` and by `estsc.shp`.
```
u16  count                    ; ≤ 152 (SPRITES_PER_TABLE_MAX)
repeat count times:
  u8   populated              ; 0 → empty sprite, nothing follows
  u16  width                  (if populated)
  u16  height
  u16  size
  u8   data[size]
```
`data` encoding (`sprite.c:blit_sprite`): byte stream
- `255 n`: skip `n` transparent pixels;
- `254`: end of line (advance to `width`);
- `253`: 1 transparent pixel;
- otherwise: opaque pixel whose palette index = the byte. Automatic line wrap when `x ≥ width`.

### 4.2 Compressed "Sprite2" sheet (`sprite.c:JE_loadCompShapes(B)`, `blit_sprite2`)
`newsh?.shp` files and the last 6 sub-tables of `tyrian.shp`. Read
**as a block** (`size = size`); lazy decoding:
```
u16  offset[N]     ; LE, N = offset[0] / 2 (= 304 in all shipped files)
...  RLE stream
```
Sprite of index `i` (**1-based**, 0 = nothing) starts at `offset[i-1]`.
Control byte `b`:
- `b == 0x0F`: end of sprite;
- `hi = b >> 4` (opaque pixels), `lo = b & 0x0F` (transparent pixels to skip first);
- `hi == 0`: after the skip, move to the next line (nominal width **12** px);
- otherwise: `hi` bytes of palette indices follow.
2×2 sprites (24×28) assemble the indices `i, i+1, i+19, i+20`
(`blit_sprite2x2`). Several offsets may point to the same place
(sharing): a writer must preserve the table as-is.

Bank → file mapping (`lvlmast.c:shapeFile[36]`, index `shapebank` 1..36):
`2 4 7 8 A B C D E F G H I J K L M N O P Q R S T U 5 # V 0 @ 3 ^ 5 9 ' %`.

38 `newsh?.shp` files verified (304 sprites each, all streams end with
`0x0F`); sizes 14,096 (`newsh0`) to 46,639 (`newshh`) B.

**`newsh$.shp`** (17,862 + 154 = 18,016 B): Sprite2 of the editor ships
**followed by 154 bytes** of encrypted `extraShips` (`editship.c:JE_loadExtraShapes`,
`JE_decryptShips`): key `[58,23,16,192,254,82,113,147,62,99]`, decryption
from end to start `s2[x] = c[x] ^ K[(x+1)%10] ^ c[x-1]` over 150 bytes,
followed by 4 check bytes (sum, difference, product+1, xor mod 256 of the
plaintext) — failure → `exit(255)`. Searched for first in the user directory.

### 4.3 `tyrian.shp` / `tyrianc.shp` (`sprite.c:JE_loadMainShapeTables`)
Container § 0.4 with `u16 shpNumb = 13` (assert) and `s32 shpPos[13]`:

| # | Contents | Format | `count` / sprites |
|---|---|---|---|
| 0 | FONT_SHAPES | 4.1 | 60 |
| 1 | SMALL_FONT_SHAPES | 4.1 | 85 |
| 2 | TINY_FONT | 4.1 | 127 |
| 3 | PLANET_SHAPES | 4.1 | 152 |
| 4 | FACE_SHAPES | 4.1 | 18 |
| 5 | OPTION_SHAPES | 4.1 | 47 |
| 6 | WEAPON_SHAPES | 4.1 | 22 |
| 7 | spriteSheet8 (player shots) | 4.2 | 304 |
| 8 | spriteSheet9 (player ships) | 4.2 | 304 |
| 9 | spriteSheet10 (power-ups) | 4.2 | 304 |
| 10 | spriteSheet11 (coins, datacubes) | 4.2 | 304 |
| 11 | spriteSheet12 (player 2 shots) | 4.2 | 304 |
| 12 | spriteSheetT2000 (T2000 ships) | 4.2 | 304 |

`tyrian.shp` = 505,983 B, offsets `[54, 6949, 11854, 15200, 135038, 180756,
319317, 350426, 382486, 415970, 442393, 471425, 493635]`; each 4.1 table
ends exactly at the next offset (verified). `tyrianc.shp` (Christmas mode,
`opentyr.c`) = 516,757 B, same structure, tables 0–7 identical in position.

### 4.4 Other `.shp` files
- `estsc.shp` (115,338 B): one 4.1 table of 9 sprites (ending images) — `mainint.c` (credits), EXTRA_SHAPES table.
- `estpa.shp` (119,552 B): valid 4.1 table of 152 sprites — **not referenced** by the code.
- `user1.shp`, `user2.shp` (27,674 B each, different contents): start with `u16 1` + `populated=0`; **not referenced** by the code; exact format not established.

---

## 5. `palette.dat` (`palette.c:JE_loadPals`)
24 palettes × 256 colors × 3 bytes (R, G, B) = **18,432 B** (verified;
`assert(palette_count == 24)`). **VGA 6-bit** components (max observed 63).
8-bit conversion: `c8 = (c6 << 2) | (c6 >> 4)`. No header.
Associations: `pcxmast.c:pcxpal[14] = {0,7,5,8,10,5,18,19,19,20,21,22,5,23}`
(palette of each `tyrian.pic` image), `facepal[12]`.

## 6. `tyrian.pic` (`picload.c:JE_loadPic`)
Container § 0.4: `u16` (read but ignored, equals 14 = `PCX_NUM`) + `s32 pcxpos[14]`.
Each entry = **raw PCX RLE image data, with no header or palette**,
320×200, 8 bpp:
- byte `b` with `(b & 0xC0) == 0xC0` → repeat the next byte `b & 0x3F` times;
- otherwise literal pixel.
Size 367,161 B; offsets `[58, 19221, 41187, 52889, 88724, 102177, 114291,
159006, 192782, 243378, 279786, 282727, 318153, 340264]`. Each image decodes
exactly 64,000 pixels and leaves **1 residual byte** unconsumed at the end
of the entry (keep it). Palette = `palettes[pcxpal[n]]`. 1-based index on the
caller side (`]P`, `]U` in `levelsN.dat`).

## 7. Standard PCX files (`pcxload.c:JE_loadPCX`)
`tshp2.pcx` (62,168 B): ZSoft PCX v5, 8 bpp, 1 plane, 320×200, `bytesPerLine=320`.
The reader skips the 128-byte header, decodes the RLE (same rule as § 6) and,
if the byte at `size-769` equals `12`, reads the 256×RGB **8-bit** palette (not
rescaled). `shipedit.pcx` (13,450 B, same format): **not referenced** by the code.

## 8. Music: `music.mus` (`loudness.c:load_music`, `lds_play.c:lds_load`)
Container: `u16 song_count` (= **41**) + `u32 song_offset[41]` (first = 166).
Size 153,482 B. Each song is in **LOUDNESS (LDS)** format:
```
u8   mode            ; 0..2, otherwise rejected
u16  speed
u8   tempo
u8   pattlen
u8   chandelay[9]
u8   regbd
u16  numpatch
patch[numpatch] (46 B each) :
  u8 mod_misc, mod_vol, mod_ad, mod_sr, mod_wave,
     car_misc, car_vol, car_ad, car_sr, car_wave,
     feedback, keyoff, portamento, glide, finetune, vibrato, vibdelay,
     mod_trem, car_trem, tremwait, arpeggio;
  u8 arp_tab[12]; u16 start; u16 size; u8 fms; u16 transp;
  u8 midinst, midvelo, midkey, midtrans, middum1, middum2
u16  numposi
position[numposi][9] (3 B): u16 patnum (byte offset; /2 in memory) ; u8 transpose
u16  (number of digital sounds — ignored, fseek +2)
u16  patterns[]      ; up to the end of the entry: (remaining_size)/2 words
```
Verified example, song 0: 4,255 B, mode 2, 13 patches, 14 positions. If the remaining
size is odd, the last byte is not read (keep it). Song index
= `musmast.c`; `]M NN` and `levelSong` are 1-based. The `.mid` files in
`data/midi/` are an external replacement (FluidSynth), outside the original format.

## 9. Demos: `demo.1` … `demo.5` (`mainint.c:load_next_demo`, `replay_demo_keys`)
30-byte header:

| Off. | Type | Field |
|---|---|---|
| 0 | u8 | episode (`JE_initEpisode`) |
| 1 | char[10] | `levelName` |
| 11 | u8 | `lvlFileNum` |
| 12 | u8 ×6 | front gun, rear gun, super_arcade_mode, left sidekick, right sidekick, generator |
| 18 | u8 ×2 | sidekick_level, sidekick_series (ignorable) |
| 20 | u8 | initial_episode_num |
| 21 | u8 ×3 | shield, special, ship |
| 24 | u8 ×2 | front / rear gun power |
| 27 | u8 ×3 | unused |
| 29 | u8 | `levelSong` |
| 30 | **u16 BE** | initial delay (frames) |
| 32 | (u8 keys, **u16 BE** wait)* | until EOF |
`keys` bits: 0 up, 1 down, 2 left, 3 right, 4 button0, 5 button3, 6 button1, 7 button2.
Sizes: 2,745 / 2,193 / 1,974 / 699 / 1,122 B (`size-30 ≡ 0 mod 3` → the
last triplet is incomplete: 1 tail byte, partially read before `feof`).

## 10. Sounds: `tyrian.snd`, `voices.snd`, `voicesc.snd` (`nortsong.c:loadSndFile`)
Container: `u16 count` + `u32 offset[count]`; data = **signed 8-bit PCM,
mono, 11,025 Hz** (`SDL_BuildAudioCVT(AUDIO_S8, 1, 11025, …)`), with no header.
- `tyrian.snd`: 271,689 B, `count` must be **31** (`SFX_COUNT`); each sound ≤ 65,535 B.
- `voices.snd` (132,767 B) / `voicesc.snd` (188,275 B, Christmas): `count` = **9** (`VOICE_COUNT`);
  **the last 100 bytes of each voice are ignored** ("bad data") — keep them for the round-trip.

## 11. Encrypted texts
- **`cubetxtN.dat`** (`game_menu.c:load_cube`): encrypted Pascal strings (§ 0.3).
  Datacube k = after the k-th `*` line; the `*` line carries the face number
  (`str_pop_int(&buf[4])`, 1-based); then title, header, text lines
  up to the next `*`; empty line = new paragraph.
  Sizes: 35,780 / 8,109 / 12,277 / 61,763 / 7,024 B; 911 / 189 / 298 / 1,660 / 177 strings.
- **`tyrian.cdt`** (1,078 B, `mainint.c` credits): 126 encrypted strings, the first being "`* CREDITS *`".
- **`tyrian.hdt` [4..21 174)**: 677 encrypted strings (see § 2.1, not read).
- Displayed texts now come from `data/locales/*.txt` (UTF-8, in-house format, out of scope).

## 12. Miscellaneous
- **`tyrend.anm`** (3,315,848 B, `animlib.c`): Deluxe Paint Animation "LPF".
  Header: `"LPF "`, ignored `u16`, `u16 nlps` (=51), `u32 nRecords` (=112);
  palette 256 × BGRx (8-bit) at 0x100; page headers (`u16 baseRecord,
  nRecords, nBytes`) at 0x500; pages of 0x10000 B starting at 0xB00; each
  page: 6-byte header + 2 skipped bytes + `u16 recordSizes[nRecords]` + data;
  RunSkipDump compression (`animlib.c`, stops at `0x80 0x00 0x00`).
- **`exitmsg.bin`** (4,000 B): 80×25 text screen (character, attribute); the `JE_drawANSI` call is **commented out** (`varz.c`) → unused.
- **Non-Tyrian files in the `data/` folder** (Enhanced-specific, JSON/text): `sprite_manifest.json`, `locales/`, `midi/`, `reference/`.

## 13. Redistribution rights (to be validated legally)

| Item | Origin | Status |
|---|---|---|
| `*.lvl, levels*.dat, *.hdt, *.shp, shapes*.dat, palette.dat, music.mus, *.snd, *.pic, *.pcx, *.cdt, cubetxt*.dat, demo.*, tyrend.anm, exitmsg.bin` | Tyrian 2000, Eclipse Software / Epic MegaGames | **Freeware since 2004** (README.md, packaging/README.txt); source: camanis.net/tyrian/tyrian2000.zip. Freeware ≠ free license: no written license in the repository; not covered by the code's GPL (`COPYING`). |
| `data/soundfont.sf2` (5.97 MB) | TimGM6mb (Tim Brechbill, David Bolton) | GPL v2; text and notice in `rewrite/licenses/TimGM6mb-LICENSE.txt`, shipped in the packages. |
| `data/midi/*.mid` (41) | karlsitretro.com, arrangements after A. Brandon & A. Molnar (commit 211f39eb) | **Unclear rights**: third-party transcriptions of copyrighted music, no license attached. |
| `data/hd/upscaled/` | Generated by xBRZ (tool) from the Tyrian data | Absent from the repository (.gitignore); derived from the freeware data. |
| `data/reference/fonts/*.ttf/.woff2` | Fonts derived from the glyphs of `tyrian.shp` | Derivative work of the freeware data; no license of its own. |
| `user1.shp`, `user2.shp`, `estpa.shp`, `shipedit.pcx` | Tyrian 2000 | Freeware, not used by the code. |

## 14. Verified summary

| File(s) | Size (B) | Key structure | Verification |
|---|---|---|---|
| tyrian1..5.lvl | 538 856 / 381 187 / 393 938 / 937 303 / 521 137 | `u16 n` + `s32[n]`; pairs (header+events, 22,368 B map); last = items (ep. 4–5) | all entries parsed, end of each level = next offset |
| levels1..5.dat | 9 600 / 5 534 / 6 052 / 11 116 / 4 257 | encrypted Pascal strings | exact round-trip |
| tyrian.hdt | 295 069 | `s32` 21 174 + 677 strings + items block 273,895 (273,818 read + 77 B unread tail) | name alignment verified |
| shapesw/x/y/z/).dat | 172 480 / 217 504 / 206 752 / 318 976 / 118 720 | 600 × (u8 blank [+672]) + 520 B | exact consumption |
| tyrian.shp / tyrianc.shp | 505 983 / 516 757 | 13 sub-tables (7 simple + 6 Sprite2) | table ends = offsets |
| newsh?.shp (38) | 14 096 – 46 639 | Sprite2, 304 sprites | all end with 0x0F |
| estsc.shp / estpa.shp | 115 338 / 119 552 | simple table 9 / 152 | exact consumption |
| palette.dat | 18 432 | 24 × 256 × RGB 6-bit | max 63 |
| tyrian.pic | 367 161 | 14 RLE images 320×200 | 64,000 px each, +1 residual B |
| tshp2.pcx / shipedit.pcx | 62 168 / 13 450 | PCX v5 8 bpp + palette (marker 12) | headers verified |
| music.mus | 153 482 | 41 LDS songs | headers parsed |
| tyrian.snd / voices.snd / voicesc.snd | 271 689 / 132 767 / 188 275 | 31 / 9 / 9 PCM S8 11,025 Hz samples | counts OK |
| cubetxt1..5.dat | 35 780 / 8 109 / 12 277 / 61 763 / 7 024 | encrypted strings | exact round-trip |
| tyrian.cdt | 1 078 | 126 encrypted strings | exact round-trip |
| demo.1..5 | 2 745 / 2 193 / 1 974 / 699 / 1 122 | 30 B header + BE delays | |
| tyrend.anm | 3 315 848 | LPF, 51 pages, 112 records | header verified |
| exitmsg.bin | 4 000 | 80×25×2 | unused |

### Pitfalls for a byte-exact writer
1. Preserve all **unread** areas: 520-byte tail of `shapes*.dat`, 77 B of `tyrian.hdt`, encrypted help strings of `tyrian.hdt`, residual byte of each `tyrian.pic` image, last 100 bytes of each voice, `mapSh[*][72..127]`, `itemNum`, the header `u16` of `tyrian.pic`.
2. Store booleans (`blank`, `populated`, `stop`) as raw bytes.
3. `string[30]` names: keep `nameLen` and the 30 bytes (space padding) as-is.
4. Sprite2: shared offsets and stream order must be kept; do not re-encode the RLE (several valid encodings).
5. `mapSh` big-endian, demo delays big-endian, everything else little-endian.
6. Event read order: `eventdat3, eventdat5, eventdat6, eventdat4`.
