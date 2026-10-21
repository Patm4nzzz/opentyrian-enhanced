# Player Guide — OpenTyrian2000 Enhanced (Godot version)

This guide covers the Godot rewrite of OpenTyrian2000 Enhanced: installation, controls, game
modes, options, mods, personal files and known limitations. It does not cover the original C
version, whose `README.md` describes different keys and options.

---

## 1. Installation

### Downloads

Each release (current: [v1.0-rc1](https://github.com/Patm4nzzz/opentyrian-enhanced/releases/tag/v1.0-rc1))
offers five files; the version is part of each name:

| File | Size (v1.0-rc1) | Content |
|---|---:|---|
| `OpenTyrian2000-Enhanced-<version>-windows-x86_64.zip` | 703.8 MB | Windows, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-<version>-linux-x86_64.tar.gz` | 683.2 MB | Linux / Steam Deck, full |
| `OpenTyrian2000-Enhanced-<version>-windows-x86_64-lite.zip` | 197.1 MB | Windows, without `upscaled/` |
| `OpenTyrian2000-Enhanced-<version>-linux-x86_64-lite.tar.gz` | 185.5 MB | Linux / Steam Deck, without `upscaled/` |
| `OpenTyrian2000-Enhanced-<version>-hd-pack.zip` | 506.5 MB | `upscaled/` alone, for a lite download |

Download one game archive; the HD pack is only useful with a lite one.

### Package contents

Each archive contains an `OpenTyrian2000-Enhanced/` folder with:

| Item | Purpose |
|---|---|
| `OpenTyrian2000-Enhanced.exe` (Windows) or `OpenTyrian2000-Enhanced.x86_64` (Linux) | the game |
| `.pck` file | game resources, keep it next to the executable |
| `tyrian_godot.dll` (Windows) or `libtyrian_godot.so` (Linux) | game engine; without it, no game and no mods |
| `data/` | Tyrian 2000 data read by the game (levels, sprites, palettes, translated texts) |
| `COPYING` | code licence |
| `DATA-NOTICES.txt` | notice about the game data |
| `upscaled/` | enlarged images for the "HD (AI)" graphics mode; without this folder the mode is not offered |

Keep all these items together, in the same folder.

**Lite download.** The `-lite` archives are the same game without `upscaled/`, several hundred MB
smaller. Everything plays the same; only the "HD (AI)" graphics mode is missing. To add it later,
download the HD pack of the same release (`OpenTyrian2000-Enhanced-<version>-hd-pack.zip`) and extract it
into the game folder, so that `upscaled/` sits next to the executable. Delete `upscaled/` to remove
it again.

### Windows

1. Extract `OpenTyrian2000-Enhanced-<version>-windows-x86_64.zip` (or its `-lite` variant).
2. Open the `OpenTyrian2000-Enhanced` folder and run `OpenTyrian2000-Enhanced.exe`.

### Linux and Steam Deck

1. Extract `OpenTyrian2000-Enhanced-<version>-linux-x86_64.tar.gz` (or its `-lite` variant), for example:
   `tar -xzf OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz`
2. Run `./OpenTyrian2000-Enhanced.x86_64` from the `OpenTyrian2000-Enhanced` folder.
   If the file is not executable: `chmod +x OpenTyrian2000-Enhanced.x86_64`.

On Steam Deck, the console control profile is applied automatically on first launch when Steam
provides the `SteamDeck=1` environment variable (see §2.5).

### macOS (experimental, unsigned)

`OpenTyrian2000-Enhanced-<version>-macos-universal.zip` is built on demand by the project's CI and is
not yet part of a release. It holds a single app, `OpenTyrian2000 Enhanced.app`, for Intel and
Apple Silicon Macs (macOS 11 or later), with the HD (AI) art included.

1. Unzip it (double-click the `.zip`); move the app to Applications if you like.
2. The app is not signed with an Apple Developer ID nor notarized: the first time, **right-click**
   it, choose **Open**, then **Open** again. Or try to open it, then go to **System Settings ›
   Privacy & Security** and click **Open Anyway** (the only way on macOS 15 and later). Or run
   `xattr -dr com.apple.quarantine "/Applications/OpenTyrian2000 Enhanced.app"` in Terminal.

Everything the game ships with (`data/`, example `mods/`, `upscaled/`, `COPYING`,
`DATA-NOTICES.txt`) is inside the app, in `OpenTyrian2000 Enhanced.app/Contents/Resources/`
(right-click the app › Show Package Contents). **Never add or change files inside the app**: that
breaks its signature and macOS will then refuse to open it. Your own files live outside it (§6).

**HD (AI) art on macOS.** The game looks for `upscaled/` first inside the app
(`Contents/Resources/upscaled`), then in `~/Library/Application Support/OpenTyrian2000 Enhanced/upscaled`.
The macOS download already includes it; should a lite macOS variant exist, extract the HD pack into
that second folder, never into the app.

### Game data location

The game looks for its data in this order:

1. the folder given by the **`TYRIAN_DATA`** environment variable (optional);
2. otherwise the **`data/` folder next to the executable** (macOS: `Contents/Resources/data` inside the app).

It can also be given at launch, after `--`:
`OpenTyrian2000-Enhanced.exe -- --data=C:\path\to\data`.

The window opens at 960×600; the game's 320×200 picture is enlarged by whole-number factors.

---

## 2. Controls

Player 1 and player 2 controls can be changed in the **Controls screen (F9)**, see §2.6.
Keys are identified by their **physical position** on the keyboard: player 2's W A S D keys are
**Z Q S D on an AZERTY keyboard**.

### 2.1 Keyboard — in game

| Command | Player 1 | Player 2 |
|---|---|---|
| Up | Up arrow | W (Z on AZERTY) |
| Down | Down arrow | S |
| Left | Left arrow | A (Q on AZERTY) |
| Right | Right arrow | D |
| Fire | Space | F |
| Change fire (rear weapon mode) | Enter | G |
| Left sidekick | Ctrl | R |
| Right sidekick | Alt | T |
| Pause / resume | P (any key resumes) | — |
| In-game menu (volumes, effects, game speed, quit) | Esc | — |
| In-game help | F1 | — |

**Pause (P)**: the game freezes (no simulation tick, so replays and golden masters are not
affected), the music plays at half volume and "PAUSED" is shown in the middle of the play area, as
in the C version. Any key, gamepad button or mouse click resumes (Ctrl, Alt, Shift and the Windows key alone do
not); Esc pressed to resume does not leave the level. The game also pauses when the window loses
focus. F2 pauses as well as P. The pause key can be changed in the Controls screen (Setup > Key Bindings).

**In-game menu (Esc, or Back / LB on a gamepad)**: the C version's pause menu, drawn over the frozen,
darkened level. The game does not move while it is open and resumes on the exact tick where it
stopped. Up / Down pick a line, the mouse works too (hover, left click, right click closes):

- **Music Vol.** / **Sound Vol.**: Left / Right change the volume by one notch, Enter mutes or
  unmutes (the bar turns grey); same settings as Setup > Sound, saved.
- **Visual Effects**: Off / Basic / Extended (the C's Visual FX preset; Extended adds the advanced
  lighting — shot halos, invulnerability aura). Saved.
- **Game Speed**: Slug Mode, Slower, Slow, Normal, Turbo — the C's speeds (Normal = 35 ticks per
  second, Turbo ≈ 49, Slow ≈ 28, Slower ≈ 23, Slug ≈ 14). Only the pace changes, never the game
  itself. Saved, applied to every level.
- **Return to Game** (or Esc) closes the menu.
- **Quit** asks "Are you sure you want to exit?" (**NO** selected; Left / Right or Tab choose, Y / N
  answer at once). **YES** abandons the level: in a campaign you go back to the game menu of the
  last save ("LAST LEVEL"), in Endless the run ends (score, leaderboard). **NO** returns to the menu.

On the "Game Over" screen Esc opens nothing: like any key, it closes the screen.

**In-game help (F1)**: the original help panels (power bar, weapons, sidekicks, shield/armor; the
two-player version in two-player games) over the frozen level; any key or click returns to the game.
**Ctrl+F1** shows or hides the rewrite's technical information (tick, scroll position), which used
to be on F1.

### 2.2 Mouse — in game

| Mouse action | Effect |
|---|---|
| Movement | moves the ship |
| Left button | fire |
| Right button | both sidekicks |
| Middle button | change fire |

### 2.3 Gamepad — in game

The first gamepad controls player 1, the second one player 2. Xbox-style layout (button names
may differ by brand).

| Command | Button(s) |
|---|---|
| Movement | d-pad or left stick |
| Fire | A or right trigger (R2/RT) |
| Change fire | B (the left trigger L2/LT is bullet time, §2.6) |
| Left sidekick | X |
| Right sidekick | Y |
| Pause / resume | Start or right shoulder button (RB/R1) (any button resumes) |
| In-game menu (Quit asks for confirmation) | Back / Select (View) or left shoulder button (LB/L1), like the C's buttons 4 and 5 |

- **Left stick**: by default, as in the C version, it acts as the d-pad (all-or-nothing past
  half-way). With **Setup → Joystick → Analog** on, it becomes analogue as the C version's analogue
  joystick mode (`joystick.c`): the ship's speed follows how far the stick is pushed. Dead zone: about **15 %** of its travel (threshold 5 of
  the C version); full tilt moves faster than the d-pad, as in the C version's analogue mode.
  The d-pad then counts as the stick at full tilt. **Setup → Joystick** sets, per gamepad, Analog on/off (off: the
  stick acts as a d-pad past half-way), Sensitivity and Threshold (0 to 10), and every button or
  axis assignment (§4.2).
- **Dead zone** of the triggers in game: **0.35** (35 % of their travel).

### 2.4 Menus

The same actions work in every ported menu.

| Action | Keyboard | Gamepad |
|---|---|---|
| Up / Down / Left / Right | arrows | d-pad or left stick |
| Confirm | Enter (or keypad Enter); Space on most screens | A or L1/LB |
| Back / cancel | Esc | B or R1/RB |
| Delete (name entry) | Backspace | — |
| Previous / next page | Page Up / Page Down | — |
| Yes / No ("Quit" box) | Y / N | — |
| Switch player (Controls screen) | Tab | Y |
| Defaults (Controls screen) | Delete | X |

Title screen keys:

| Key | Effect |
|---|---|
| **M** | side modes screen |
| **2** | two-player Endless mode |
| letters | secret codes of the original (see §3.6) |
| Esc | asks for confirmation before quitting the game |

### 2.5 Steam Deck

On first launch on Steam Deck (`SteamDeck=1` variable and no `controls.cfg` file saved yet), the
game applies this profile:

| Command | Control |
|---|---|
| Movement | left stick or d-pad |
| Fire | R2 or A |
| Change fire | B (L2 is bullet time) |
| Left sidekick | X |
| Right sidekick | Y |
| Pause in a level | Start or RB/R1 |
| Back (menu) | B or R1 |
| In-game menu | Back (View) or LB/L1 |

The back grips (L4, L5, R4, R5) are not seen by the game: to use them, map them to A, B, L1 or R1
in the Steam Input controller configuration.

As soon as you save the Controls screen (F9), your own `controls.cfg` file replaces this profile.

### 2.6 Function keys

**In a level** (same keys as the original C version; every one can be changed in the Controls
screen):

| Key | Effect |
|---|---|
| **F1** | in-game help of the original; **Ctrl+F1** shows or hides technical information |
| **F2** | pause (P too) |
| **F3** | achievements list over the game: Up/Down scroll, Left/Right change category; F3 again closes it (it closes F4) |
| **F4** | STATISTICS table over the game (this run and lifetime); F4 again closes it (it closes F3) |
| **F5** | quick save to the "QUICK SAVE" slot of the Load screen (campaign only) |
| **F6** | DPS counter (damage per second) at the bottom left of the playfield |
| **F7** | damage numbers on or off |
| **F8** | sound on / off; works everywhere, in and outside a level, except while typing a name or capturing a key |
| **F9** | quick load: asks "Quick load? Y / N / F9", Y or F9 loads, N or Esc cancels |
| **F10** | records a replay (see §3.1) |
| **F11** | gamma: four levels (saved) |
| **F12** | screenshot `screenshot_NNNN.bmp` in your user folder (§6), numbered after the last existing one |
| **Left Shift** | **bullet time**: 4 s of slow motion, then a 15 s cooldown (never saved). Gamepad: left trigger. Mouse: side button 4. The original uses Tab here; Left Shift is a deliberate choice. If a player's game key is Left Shift, the game key wins; Right Shift does nothing |

**Outside a level** (menus, shop, title): F2 cycles the CRT filter, F3 the graphics mode, F9 opens
the Controls screen and F10 the Display options screen. All of them are also in the Setup menu
(title screen or shop): Setup > Graphics and Setup > Key Bindings.

**Controls screen** (Setup > Key Bindings, or F9 outside a level): Up/Down picks the command; Enter, then press the new key or gamepad
button (the cancel key, Esc by default, cancels the capture); Tab (or Left/Right) switches between
player 1 and player 2; Delete restores the defaults for both players; Esc saves and returns to the
title (or to the paused game, see above). A new key
replaces the command's keyboard keys without touching its gamepad buttons, and vice versa. Axes
(stick, triggers) cannot be changed.
The lines after the players' commands are shared by both players and keyboard only: Help (F1 by default),
Cancel key capture (Esc by default), Pause (F2 by default; P always pauses too, and the gamepad Start
button), Gamma, and the level shortcuts of the table above. A key that would lock you out is
refused with the message "Key refused: already used": the cancel key cannot be Enter, Up, Down,
Tab, Delete, a game key of either player or another shared key; a shortcut cannot take a game key of
either player or a key already used by another shortcut; P is reserved. The only exception is Left
Shift, which a player may use as a game key (the game key then wins over bullet time). These keys are
saved in `controls.cfg` with the others. The list scrolls with the selected row.

---

### 2.7 Launch options and developer keys

Add them after `--` on the command line (`OpenTyrian2000-Enhanced.exe -- --speed=5`), as in the C version
(`params.c`):

| Option | Effect |
|---|---|
| `--speed=N` | forces the game speed without saving it: 1 slug, 2 slower, 3 slow, 4 normal, 5 turbo, 6 hidden turbo (no wait: at the display's pace) |
| `--devmode` | enables the Ctrl+Shift developer keys (off otherwise) |

With `--devmode`, in a level: **Ctrl+Shift+G** god mode (the ship fires continuously, drifts down
and bounces from edge to edge, like the C's `constantPlay`; again to stop it), **Ctrl+Shift+H**
armor back to 28, **Ctrl+Shift+C** +10,000 credits, **Ctrl+Shift+N** level completed and won
(debriefing, campaign continues), **Ctrl+Shift+U** unlocks every achievement. These are testing
tools: nothing changes without `--devmode`.

## 3. Game modes

### 3.1 Main menu

| Entry | What it does in this version |
|---|---|
| Start New Game | choice of the mode, then of the episode and the difficulty (see §3.2) |
| Load Game | resumes a game from a save (see §6) |
| High Scores | high score table |
| Instructions | game help |
| Setup | settings (see §4.2) and mod manager |
| Achievements | unlocked achievements (read from the save) |
| Replays | list of replay files (the ones you record, see below, and those of the C version); Enter plays the selected replay back ("PLAY" at the top right of the playfield), Esc or the end of the replay returns to the title. Arcade replays (one and two players) play back like the others |
| Demo | plays the next of the five demos (`demo.1` … `demo.5`, in turn); any key, click or gamepad button returns to the title. The demo also starts by itself after 30 seconds without input on the title screen |
| Quit | asks for confirmation, then quits |

**Recording a replay (F10 in a level).** The key arms the recording: "Recording starts at next level".
Press it again before the level ends to cancel ("Recording cancelled"). At the start of the next
level "REC" appears at the top right of the playfield and "Recording started" is shown; press F10
again, finish or leave the level to end the recording. The file is saved as
`replays/YYYYMMDD-HHMMSS.rep` (folder in §6) and appears in the Replays screen. A replay holds the
level, the ship's equipment, the mutators and the position and buttons of your ship at every frame,
so it plays back the same way in this version and in the C version (same file format). Normal game,
Arcade (one player), two players and mutators can be recorded; Galaga, Super Arcade, Timed Battle,
Super Tyrian, Endless and a game with a mod installed cannot (the replay would not play back the
same): "Mode not recordable" is shown and no file is written. Not available during a replay or the
demo.

### 3.2 Campaign

"Start New Game" opens the screens of the original, image for image:

1. **Mode** ("PLAYERS"): 1 Player Full Game, 1 Player Arcade, 1 Player Endless Mode, 1 Player
   Timed Battle, 2 Player Arcade, 2 Player Endless Mode. "Modem/Network Game" is greyed out and
   refused, as in the original when the network game is not available.
2. **Episode** (greyed out if `tyrianN.lvl` is missing from `data/`), or for Timed Battle the
   **battle** (Deliani, Space Station, Savara).
3. **Difficulty**: Easy, Normal, Hard. Hidden levels as in the original: **Shift+G** shows
   Impossible, then **Shift+]** shows Suicide, then typing **L O R D** (or holding the four keys)
   shows Lord of Game.

**Esc** goes back one screen (difficulty → episode → mode → title). Full game: starting cash of the
episode (10,000 to 30,000). Arcade: Stalker ship, no cash, short "Game Menu" (Start Level, Options,
Quit). Two players: Silver Ship and Dragonwing, difficulty one level higher, player 2 on the mouse.
Timed Battle: the chosen battle, then back to the title.

Levels follow one another with the shop and the results screen between levels, then move on to
the next episode at the end of an episode.

**Results**: after each level, the results screen (money, enemies destroyed, data cubes) ends on
"Press a key"; then the **level statistics** screen (Enhanced edition) shows enemies destroyed,
the share of damage by weapon (front gun, rear gun, sidekicks, special; 5 % and more), damage
taken with the share absorbed by the shield, credits earned and level time. Enter or Esc (gamepad:
A, B, L1 or R1) closes it.

**Shop** ("Game Menu", same entries and keys as the original):

| Entry | What it does |
|---|---|
| **Data** | data cubes picked up in the previous level (up to 4): pick one, Enter reads it, Up / Down scroll the text, Esc returns to the list |
| **Ship Specs** | the current ship's specification sheet; any key returns to the menu |
| **Upgrade Ship** | Ship, Front Gun, Rear Gun, Shield, Generator, Left and Right Sidekick: the hovered item is fitted live and its price shown; in the weapon categories the fitted guns fire in the demo area on the left (power level, stars and shots as in the original). Enter chooses, "DONE" pays (the owned item is resold at its value), Esc cancels and refunds. On guns, Left / Right lower / raise the power level (cost shown per level); `/` cycles the rear weapon mode. Items you cannot afford are skipped |
| **Options** | Load Game / Save Game (see below) |
| **Start Level** | opens the "Next Level" menu: an animated map shows the route from the current planet to the hovered destination; choose the next destination |
| **Quit Game** | asks for confirmation, then returns to the title screen (Esc on the Game Menu does the same) |

Specials are not sold in the shop, in the original either: they are picked up during levels.

**Saving**: in the shop, choose **Options**, then **Save Game**, a slot (1 to 10), type a name
(14 characters, Backspace deletes, Enter confirms, Esc cancels) and confirm with **YES**. The game
resumes at the shop where it was saved. **Load Game** is in the same Options menu and on the title
screen. Slot 11, **LAST LEVEL**, is written automatically before every level: it can be loaded,
not overwritten.

**Game over**: if your ship is destroyed in a normal level, "Game Over" is shown until you press
any key, fire, a sidekick button or a mouse button, as in the C version; the level then starts again with the ship, weapons and money you had when you entered it
(the money earned during the lost attempt is lost), as in the original. In a bonus level, being
destroyed simply ends the level.

**Bonus games**: two shops ask "Do you have the special ship?" (episode 1, before ** ALE **;
episode 4, before SQUADRON). Their "Next Level" menu shows **Bonus Game**: the first destination is
the bonus game, the second ("Skip It") the normal campaign. In the bonus game you fly the Stalker
21.126 with an Atomic RailGun, Galaga style, with lives and no cash; it has no end: it stops when
all your lives are lost. You then return to the same shop with the ship, weapons and money you had
before, as in the original. Your own equipment is never lost.

### 3.3 Side modes (M key on the title screen)

| Mode | Start |
|---|---|
| Two players | episode 1 |
| Arcade | episode 1 |
| Super Arcade 1 | episode 1 |
| Galaga | episode 1 |
| Timed Battle 1 | episode 1 |
| Timed Battle 2 | episode 5 |

On this screen: Up/Down picks a row; on the "Mode" row, Left/Right (or Space) changes mode; on the
other rows, Left/Right/Space ticks or unticks; Enter starts the game; Esc returns to the title.

**Mutators** (can be combined): Fast Enemies, Armored, Bullet Hell, Glass Cannon, No Powerups,
One Life, Mirror Mode, Risk / Reward.

In two-player mode, player 2 uses their keys (§2.1) or the second gamepad.

### 3.4 Endless (Start New Game, or 2 on the title screen)

- "1 Player Endless Mode" / "2 Player Endless Mode" in the mode screen; **2** on the title screen
  still starts the two-player mode directly. The **E** shortcut is gone: E is the first letter of
  the ENEMY and ENGAGE codes (§3.6).
- Levels follow one another in waves; the wave number is shown in game. You start with 3 lives.
- **Combos** are active ("Combo" option of the Setup menu, on by default): chaining kills raises
  the score.
- **Resume**: the run is saved at each new wave. On the next start, if a run is in progress, a
  screen offers: **Enter** = resume, **Esc** = new run.
- **Game over**: name entry if the score makes the leaderboard (Backspace deletes, Enter
  confirms, Esc leaves the name "PLAYER"), summary (Enter or Esc), then the top-10 leaderboard:
  **Enter or R** = instant new run, **Esc** = back to the title.
- Leaving a level (Esc, or Back on a gamepad) **ends the run**, as in the original: score and
  leaderboard, nothing to resume. A run can only be resumed after an abrupt stop (window closed,
  crash), by the screen above.
- **Leaderboard outside a run**: open **High Scores** from the title screen; the Endless leaderboard
  is the last page (Right after the last Timed Battle page, or Left on the first page).


### 3.5 DESTRUCT

**DESTRUCT** is the artillery mini-game of the original. Open it by typing **DESTRUCT** on the
title screen (§3.6), or with the *Destruct* entry of the modes screen (**M** on the title screen).

#### Modes

The five modes, as in the original: 5-Card War, Traditional, Heli Assault, Heli Defense and Outgunned.

Up / Down picks a mode; Enter starts a round; Esc returns to the title.

#### Controls (round)

| Action | Player 1 | Player 2 |
|---|---|---|
| Move left / right | C / V | Left / Right (or numpad 4 / 6) |
| Aim up / down | A / Z | Up / Down (or numpad 8 / 2) |
| Change vehicle | Left Alt | \ or numpad 5 |
| Fire | X or Left Shift | Insert, Enter, numpad 0 or numpad Enter |
| Previous weapon | Left Ctrl | Page Up (or numpad 9) |
| Next weapon | Space | Page Down (or numpad 3) |

Fixed keys: **F10** toggles the computer for player 1, **F11** the computer for player 2,
**Backspace** builds a new map, **P** pauses, **F1** shows the help, **Esc** ends the round and
returns to the mode selection; a second Esc returns to the title.

---

### 3.6 Secret codes of the title screen

Typed on the title screen (letters only, no Enter), as in the original:

| Code | Effect |
|---|---|
| STEALTH, STORMWIND, TECHNO, ENEMY, WEIRD, UNKNOWN, NORTSHIPZ, LIZARD, PRETZEL | Super Arcade game with the matching special ship: episode and difficulty choice, ship screen, then a one-player game with that ship, its weapon and its special weapon, one difficulty level higher |
| ENGAGE | Super Tyrian: "whoa" screen effect, warning texts, episode choice, then Stalker 21.126 with the Atomic RailGun on Lord of Game (Suicide if Scroll Lock is held) |
| DESTRUCT | the Destruct mini-game (§3.5) |


## 4. Options

### 4.1 Display options (F10 outside a level)

| Option | Effect | Saved? |
|---|---|---|
| Graphics | **Original** (original pixels), **Sharp pixel art**: xBRZ filter that rounds edges while keeping the pixel-art style, computed at display time, no file or dedicated GPU needed, or **HD (AI)**: every image enlarged four times in advance by an AI model trained on pixel art (4x-PixelPerfectV4); the scenery of each level is enlarged as one whole picture, so no tile grid shows. Also F3 outside a level | yes |
| Scaler | pixel-art scaling algorithm applied to the 320×200 picture before it is stretched: None, 2x, Scale2x, hq2x, 3x, Scale3x, hq3x, 4x, hq4x. The factor also sets the window size in windowed mode | yes |
| Scaling Mode | how the picture is fitted in the window: Center (native size, centred), Integer, Fit 8:5, Fit 4:3, Fit 16:9, Fill | yes |
| CRT filter | six modes: Off, Scanlines, CRT Plus, CRT Premium (glow and colour fringing), Aperture Grille (RGB stripes of a PVM), Soft (heavy glow, no curve); also F2 outside a level, which cycles them | yes |

Enter or Space changes the selected option; Esc returns to the title.

**Bullet time**: Left Shift in a level (§2.6). The game then runs at a quarter of its speed.
No colour tint (as in the original: only the ">> SLOW MOTION <<" message). It is never saved. Setup > Gameplay > Bullet Time turns it off altogether.

### 4.2 Setup menu

The Setup menu only shows options that act on this version; every change applies at once and is
saved in `setup.cfg` (§6). Full decision list per option: `docs/setup-options.md`.

| Menu | Options |
|---|---|
| Graphics | **Display**: Window, or full screen on Display 1, 2… (Enter opens the list), **Scaler**: None, 2x, Scale2x, hq2x, 3x, Scale3x, hq3x, 4x, hq4x, **Scaling Mode**: Center, Integer, Fit 8:5, Fit 4:3, Fit 16:9, Fill, **CRT**: six modes (see §4.1), **Sprites**: Original, Remastered (HD (AI)) or Upscaled (xBRZ) — the same setting as the F10 screen, **Visual Effects**: Off, Basic or Extended, **Explosion style**: Classic (original sprites only), Retro+ (default: sprites plus pixel debris), Modern (glowing fireballs, shockwave rings sized to the kill, boss sub-explosion chain) |
| Sound | **Music Volume** and **Sound Volume** (Left/Right; Enter mutes / unmutes), **Stereo** (each sound comes from where it happens on screen; off: centred), **SFX Variation** (±5 % pitch per effect), **Impact Scaling** (deeper, louder explosions for big enemies and bosses), **Music Engine**: Classic, Authentic or Enhanced (see below), **Stereo widening** (pseudo-stereo on the OPL music) |
| Gameplay | **Bullet Time** (off: Left Shift does nothing), **Freeze Frames** (on by default: enemies freeze 2–3 frames on heavy kills and bosses, a boss kill adds a slow-down, a flash and five follow-up explosions), **Damage Numbers** (OFF / ON / Colored, default Colored; F7 toggles them in a level), **Combo** (Endless; in the campaign, money stays that of the original game, never multiplied), **Auto-Fire** (both players fire continuously), **Rumble** (gamepad vibration on kills, hits and death), **Language**, **Mutators...** (the 8 mutators of §3.3 plus Reset All; they apply to every game, story included, and are saved) |
| Key Bindings | opens the Controls screen (same as F9); Esc returns to Setup |
| Joystick | gamepad settings, as the C version: **Joystick** line (player 1 or player 2 gamepad), **Analog**, **Sensitivity** and **Threshold** (Left/Right, 0 to 10), the ten assignments Up … Pause (Enter, then press a button or push an axis two thirds of the way; doing it again with the same input removes it; a key cancels; two inputs per line), **Reset** (default assignments and settings). Saved in `controls.cfg` |
| Mods | mod manager (see §5) |
| Jukebox | listen to the game's music over the starfield: Left/Right change track, S stop, R restart, X shuffle, G play this track in every level (G again: back to the level music), / sound effects (, and . choose, ; plays), Space hides the text, Esc returns to Setup |

All of the C's settings are back in Setup: Scaler, Scaling Mode, CRT, Sprites and Visual Effects in
Setup > Graphics; Explosion style in Setup > Graphics; Stereo, Impact Scaling and Stereo widening in
Setup > Sound; Freeze Frames, Damage Numbers, Rumble and Mutators in Setup > Gameplay; Joystick opens
its own screen (Gamepad), gamepads are no longer set in the Controls screen. Scaler, Scaling Mode and
CRT are also on the Display options screen (F10). Replays and recorded comparisons always play
without Freeze Frames and with the original explosions, like the C version's golden masters.

**Visual Effects** (Setup > Graphics): **Off** (pure 1995 look: no particles, no screen shake, no
advanced lighting), **Basic** (particles and shake) or **Extended**, the default, which adds the
advanced lighting — exactly the C's Visual FX presets, which the game maps to its internal effect
level (`processorType`). Screen shake follows the C's `screenshake.c`: big kills (value 1000+ / 300+ / 100+),
being hit, dying and defeating a boss shake the picture (random offset up to the current intensity,
fading by 10 % per frame); Off turns it off.
**Music Engine** (Sound menu, Left/Right or Enter), the three engines of the C version:
**Classic**, the default, plays the original AdLib soundtrack as emulated by DOSBox's OPL;
**Authentic** plays the same soundtrack through the Nuked OPL3 emulator (closer to a real chip);
**Enhanced** plays the General MIDI arrangements of `data/midi/` with the `data/soundfont.sf2`
SoundFont. The current tune restarts at once in the chosen engine, and the choice is kept in
`setup.cfg` (a choice saved by an older version, "Original OPL" or "MIDI", is read as Classic or
Enhanced). If the SoundFont or the MIDI file of a tune is missing, that tune plays in its OPL
version (a warning is written to the log): the music never goes silent. **Stereo widening** adds the C
version's pseudo-stereo (a short delay between left and right) to the Classic and Authentic music.

---

## 5. Mods

### Installing a mod

1. In **Setup → Mods**, pick **Install a mod (.zip)** and choose the mod's `.zip`, or drop the
   `.zip` on the game window while that menu is shown. The mod is copied to your `mods/` folder and
   appears in the list; a refused archive shows the reason on the line (no `mod.json`, a file path
   leaving the mod folder, a mod of the same name already installed: delete it first).
   **Open mods folder** opens that folder in your file manager. You can also copy the mod folder
   (it contains a `mod.json`) into the `mods/` folder of your personal files yourself (see §6),
   for example
   `OpenTyrian2000-Enhanced\userdata\mods\my_mod\` (macOS:
   `~/Library/Application Support/OpenTyrian2000 Enhanced/mods/my_mod/`, never inside the app).
   The example mods shipped with the game are in `rewrite/mods/` of the source repository.
   Mods written for the C version are not read as is: they show as ERR until
   converted to this version's format (`MODDING.md` §11).
2. Open **Setup → Mods**. Each mod is marked ON, OFF or ERR (error, with the message).
3. **Enter** enables or disables the selected mod; **Esc** applies (immediate reload) and returns
   to the title.

The choice is kept in `mods.cfg`; the mod's `mod.json` is never modified. For mods with the same
name, the one in your personal files wins over the one in `rewrite/mods/`.

Launch options (not remembered): `-- --mods=off` disables all mods; `-- --mods=a,b` enables only
mods `a` and `b`.

An invalid mod produces a displayed error, not a crash (limits: at most 32 mods, PNG files of at
most 4 MiB and 2048 px per side, etc. — details in `MODDING.md` §9).

### Mods shipped with the game

Five packs come with the game, disabled; enable any of them, or all five at once, in
**Setup → Mods**. Their items are sold in every shop (the shop list scrolls when a category holds
more than five items).

| Pack | Adds |
|---|---|
| Alien Ship Pack | 8 Hazudra and Zica ships |
| Bio Ships Pack | 6 bio-organic alien ships |
| Alien Weapon Pack | 6 Hazudra and Zica weapons (3 front, 3 rear) |
| Bio Weapons Pack | 6 living weapons (3 front, 3 rear) |
| Elements Weapon Pack | 6 elemental weapons (3 front, 3 rear) |

Mod ships fly with their own banking poses; mod weapons have eleven power levels like the
originals. A saved game that uses a mod item needs that mod enabled to load.

### Creating a mod

- Full reference: [`MODDING.md`](../MODDING.md).
- Template to copy: `rewrite/mod_template/` in the source repository.
- Complete example (source repository, not shipped with the game): `rewrite/mods/_retired/example_behaviors/`.
- Shipped packs to look at: the `mods/` folder next to the game (`README.md` lists them).
- Step-by-step tutorial: [`docs/FIRST_LEVEL_15_MIN.md`](FIRST_LEVEL_15_MIN.md).

---

## 6. Saves and personal files

The game stores your files in the **`userdata`** folder next to its executable, created at
first launch (like the C version on Windows, which writes next to its executable). Copying the
game folder takes your game with it; deleting `userdata` starts fresh. If the game folder is not
writable (Program Files, read-only media), the same files go to the Godot user folder instead.

**macOS** never uses `userdata/`: the inside of an app is read-only (and macOS may run a downloaded
app from a temporary read-only copy), so your files always go straight to
`~/Library/Application Support/OpenTyrian2000 Enhanced/` (in Finder: Go › Go to Folder…, paste the
path). Deleting that folder starts fresh; the app itself is never modified.

| System | Folder |
|---|---|
| Windows (fallback) | `%APPDATA%\Godot\app_userdata\OpenTyrian2000 Enhanced\` |
| Linux / Steam Deck (fallback) | `~/.local/share/godot/app_userdata/OpenTyrian2000 Enhanced/` |
| macOS (always) | `~/Library/Application Support/OpenTyrian2000 Enhanced/` |

The same folder holds the game's log, `logs/godot.log` (Windows and Linux: in the Godot user folder
above, even when `userdata/` is used).

The old `%APPDATA%\Godot\app_userdata\OpenTyrian2000 Enhanced\` folder of earlier versions is
neither read nor deleted.

| File or folder | Contents |
|---|---|
| `controls.cfg` | your controls (F9 screen) |
| `setup.cfg` | Setup menu settings (§4.2), including Visual Effects |
| `display.cfg` | "Graphics" option, CRT mode, Scaler, Scaling Mode |
| `mods.cfg` | enabled / disabled mods |
| `mods/` | your mods |
| `endless_leaderboard.json` | Endless leaderboard (10 places) |
| `endless_run.json` | Endless run in progress (deleted at game over) |
| `saves/slot_01.json` … `slot_11.json` | campaign saves (one file per slot, readable JSON, see below) |
| `opentyrian.dat` | file of the original game's format: achievements; imported once into `saves/` |
| `replays/` | replay files (recorded with F10, §3.1) listed by the Replays screen |

**Campaign**: each slot is a small JSON file that names everything by its in-game name (ship,
weapons and their power, shield, generator, sidekicks, special, money, episode, section,
difficulty). A damaged file, or one written by a newer version of the game, shows as
**Unreadable** and cannot be loaded; the other slots are not affected. The first time the game
needs its saves, if `saves/` does not exist and an `opentyrian.dat` from the C version is
at its usual location (Linux: `~/.config/opentyrian2000/`; Windows: the C game's folder,
next to or beside this one), its one-player slots are imported once; after that the C file is no
longer read for the campaign. On the very first launch its settings (language, volumes, display…)
and high scores are imported the same way, never overwriting an existing file. Alt+Enter toggles
fullscreen. Two-player campaign games are not ported: the two-player page of Load Game stays empty.

To go back to the default controls, delete `controls.cfg` (or press Delete in the F9 screen).

---

## 7. Troubleshooting

| Problem | What to do |
|---|---|
| The game cannot find its data | check that `data/` is next to the executable, or that `TYRIAN_DATA` points to a valid folder (or launch with `-- --data=<folder>`) |
| E and 2 do nothing, Setup → Mods shows "GDExtension missing: mods unavailable", no game possible | `tyrian_godot.dll` (Windows) or `libtyrian_godot.so` (Linux) is missing next to the executable: extract the full archive again |
| Linux: "permission denied" | `chmod +x OpenTyrian2000-Enhanced.x86_64` |
| macOS: "cannot be opened because the developer cannot be verified", "is damaged" or "Apple could not verify…" | the app is not signed with an Apple Developer ID: right-click › Open, or System Settings › Privacy & Security › Open Anyway, or `xattr -dr com.apple.quarantine "OpenTyrian2000 Enhanced.app"` (§1, macOS). If it appeared after files were changed inside the app, unzip the download again |
| Controls unusable after a change | Delete in the F9 screen, or delete `controls.cfg` |
| The Steam Deck profile is not applied | it only applies if `controls.cfg` does not exist yet; delete that file |
| The second gamepad does not control player 2 | player 2 uses the second detected gamepad; connect player 1's gamepad first |
| Steam Deck back grips do nothing | map them in Steam Input (see §2.5) |
| A mod is marked ERR | read the message in Setup → Mods, then §10 "Troubleshooting" of `MODDING.md` |

---

## Mouse navigation

In all menus, you can use the mouse to navigate:

- **Move** the cursor over a menu item to select it (the cursor must have moved since the last
  key press; pressing any key makes the mouse inactive until you move it again).
- **Left-click** to activate the selected item (same as pressing Enter).
- **Right-click** to go back / cancel (same as pressing Escape).
- The **scroll wheel** does nothing, as in the original C version.

The mouse pointer is hidden when over the game image and shown in the surrounding border.

## 8. Known limitations

- **Screens not ported**: the shop Options menu only offers Load / Save Game (volumes and keys are
  in Setup); the network game ("Modem/Network Game" is greyed out); the Destruct mini-game is
  available (DESTRUCT code, §3.5); the code only returns to the title if the native extension is missing.
- **Difficulty without aids only partly measured**: without any aid, the campaign bot (planned
  driving that avoids enemies, shots and obstacles) finishes all of episode 5, 6 levels of
  episode 1, 4 of episode 3 and up to 3 of episode 4, but stops at one level in each of episodes 1
  to 4 (SAVARA V, GYGES or TORM, CAMANIS, SURFACE or SIDE EXIT); played one by one with the
  equipment of the aided campaign, it finishes 38 of the 42 campaign levels.
  These are limits of the bot, not of the game.
- **Network mode** is not ported.
- **In-game rendering**: some enemies are tinted differently from the C version for a frame (colour
  flash when hit, white blink of damaged enemies, which the C version times on the real-time clock).
- **Windows**: the Windows package is built by cross-compilation. The exported
  `OpenTyrian2000-Enhanced.exe` is checked with `rewrite/scripts/test-package-windows.ps1` (headless start,
  then a golden master replayed inside the executable, identical to the recorded C run at every
  tick; the CI job `windows-native` does the same on manual dispatch only, not on every push); the windowed rendering is checked by hand with the same script.

---

## 9. Licence

- The **code** is licensed under the GPL: see the `COPYING` file included in the package.
- The **Tyrian 2000 data** (the `data/` folder, and the images, sounds and music derived from it)
  is **freeware, separate** from the code and not covered by the GPL.
- The MIDI arrangements (`data/midi/`) and the SoundFont (`data/soundfont.sf2`) come from third
  parties; their licences are not included (see `DATA-NOTICES.txt`).
- See the package's `DATA-NOTICES.txt` file. This package is not a permission to redistribute the
  data: before any redistribution, check the permissions with the rights holders.
