# OpenTyrian2000 Enhanced v1.0-rc1 — first public release candidate

**Tyrian 2000**, the 1995 vertical shoot 'em up, on a new engine: Rust runs the game, Godot 4
draws it, plays the sound and reads your keyboard, mouse and gamepad. The simulation is checked
against the previous C edition of OpenTyrian2000 Enhanced, so the game plays the same; on top of
it come an HD (AI) art mode, Endless mode, side modes and mutators, achievements, mods and a
level editor.

This is a **release candidate**: the whole game is playable from start to finish, and we want your
feedback before calling it 1.0. Please report what you find on the repository's issue tracker.

## Downloads

Pick **one** game download for your system. The full downloads contain everything; the lite
downloads leave out the HD (AI) art, which you can add later with the HD pack.

| File | Size | What it is |
|---|---:|---|
| `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64.zip` | 703.8 MB | Windows, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz` | 683.2 MB | Linux / Steam Deck, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-windows-x86_64-lite.zip` | 197.1 MB | Windows, lite (no HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64-lite.tar.gz` | 185.5 MB | Linux / Steam Deck, lite (no HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc1-hd-pack.zip` | 506.5 MB | HD (AI) art pack for the lite downloads (Windows and Linux) |

Sizes in MB (1 MB = 1,000,000 bytes). The SHA-256 checksums are listed on the release page.

- **Windows**: extract the `.zip`, open the `OpenTyrian2000-Enhanced` folder, run
  `OpenTyrian2000-Enhanced.exe`.
- **Linux / Steam Deck**: extract the `.tar.gz`
  (`tar -xzf OpenTyrian2000-Enhanced-v1.0-rc1-linux-x86_64.tar.gz`), then run
  `./OpenTyrian2000-Enhanced.x86_64` from the `OpenTyrian2000-Enhanced` folder.
  `./install-desktop-entry.sh` adds a launcher with the Tyrian icon to your applications menu.
- **HD pack**: only for the lite downloads. Extract it **into** the game folder, so that
  `upscaled/` sits next to the executable, then pick Setup > Graphics > Sprites > Remastered (or
  F10, Graphics: HD (AI)). Use the pack of the same version as the game.

Everything the game needs is in the folder: the Tyrian 2000 data (freeware), the MIDI soundtrack
and its SoundFont, the engine library. Keep the folder together. Your settings, saves, scores,
replays and mods go to a `userdata` folder next to the executable.

## What's in this release

### Downloads and your files
- The game, its folder and its executable are called **OpenTyrian2000-Enhanced**, and the
  release has **lite** downloads and a separate **HD pack** next to the full ones.
- **Your files live next to the game**, in `userdata/`: copy the game folder and your saves,
  settings, scores, replays and mods travel with it. A read-only game folder falls back to the
  usual Godot user folder.
- **Your C-edition profile is picked up** on the first launch: settings, keys, high scores,
  saves, achievements and statistics are read once from the C edition's `opentyrian.dat`
  (the original file is never changed).
- The Windows executable carries the Tyrian icon and version information; Linux gets icons and
  an optional desktop launcher.

### Close to the original, screen by screen
- **Mouse in every menu**, as in the original: shop, Setup, saves, data cubes, two-player
  Arcade input choice and more. The in-level mouse speed matches the original, including
  on Windows with display scaling at 125 % or 150 %.
- **Gamepad in menus** like the original (LB confirms like A, RB goes back like B); analog
  movement is off on a new profile, as in the original.
- **Level shortcuts of the original**: F2 pause, F3 achievements, F4 statistics, F5 / F9 quick
  save and load, F6 DPS counter, F7 damage numbers, F8 sound on/off, F10 replay recording,
  F11 gamma, F12 screenshot. Outside a level, F9 / F10 open the Controls and Display screens.
- **Record your own replays** in game (F10) and play them back from the Replays screen.
- **Episode endings** as in the original: story pages, hint, name entry, high score table,
  credits, then the next episode.
- **Secret level warps** take you to the secret level.
- Fades, sounds, colours, the pause menu, the shop, the save screens, the end-of-level screens,
  Timed Battle, two-player games and the "Visual Effects" setting were checked screen by screen,
  so that each one looks, sounds and reacts like the original.
- **Christmas prompt** in December, as in the original.

### Display
- **Full screen by default**, remembered between sessions; Alt+Enter switches cleanly; the
  window size and position are remembered in windowed mode.
- Fast start-up: no Godot splash screen, the title appears straight after the intro logos.
- In HD (AI) art mode the mouse pointer is drawn in HD too.

### Modes and options
- **Endless**: every new run draws a fresh sequence of levels; the Endless leaderboard is the last
  page of High Scores; start it from Start New Game, or press **2** for two players.
- **Side modes and mutators** (M on the title screen) work in every mode.
- **Achievements**: all of them can be unlocked, including the 46 that only the C edition
  tracked, with lifetime statistics kept in your profile.
- **Modern explosions** (Setup > Graphics > Explosions): shockwave, flash, streak and debris.
  The shockwave rings cool from white-yellow to orange to dark red, with a soft halo; small
  embers fly out of fire explosions instead of the original's hollow red sprite ring, fall
  slightly and cool down (with the original art they use the level's palette); a boss dies in a
  chain of 5 to 8 small explosions over one second before the final burst. Rendering only: the
  game plays the same, and the Original style draws the original's explosions.
- The CRT "Scanlines" filter looks like the original's.

### Mods and level editor
- Mod ships, weapons, shields and sidekicks can be **sold in the shop**, with their own images;
  saves record mod items by name, and a save that needs a disabled mod shows a clear message.
- Two example mods, **Alien Ship Pack** and **Alien Weapon Pack**, drawn in the original
  style (shipped disabled: turn them on in Setup > Mods).
- Shots drawn by a mod are visible in levels.
- Level editor: undo / redo for added enemies, invalid mods are refused with the reason when
  exporting.

### Languages
- The game is available in English, French, Spanish, German, Italian and Portuguese. The
  documentation, the package README files and the example mods are in English.

### Under the hood
- The game finds its data through a single place that also knows the layout of a macOS app; on
  Windows and Linux the data, mods and HD art sit next to the executable and your files in
  `userdata/`. There is no macOS download in this release.

## Known issues

- **Network play** is not available ("Modem/Network Game" is greyed out).
- **Shop Options menu** only offers Load / Save Game; volumes and keys are in Setup.
- **HD (AI) art**: one episode 4 level's enlarged scenery (blue rock with a bubbly texture) looks
  different from the original.
- **Some enemies flash** for one frame differently from the original when they are hit.
- **Visual effects** (particles, explosions, damage numbers) are not pixel-identical to the C
  edition; with the effects off the game is.
- **Windows** builds are cross-compiled; the packaged executable is tested by starting it and
  replaying a recorded game inside it, the windowed rendering by hand.
- **Steam Deck**: not tested on real hardware yet.
- **Coming from a development build**: settings and saves of the development builds are not
  imported. They are in `%APPDATA%\Godot\app_userdata\OpenTyrian2000 Enhanced (Godot)\` (Windows) or
  `~/.local/share/godot/app_userdata/OpenTyrian2000 Enhanced (Godot)/` (Linux); you can copy
  them into the new `userdata` folder.
- **HD pack version**: nothing stops you from installing an HD pack from another version; use the
  pack of the same version as the game.

Full player guide: [docs/PLAYER-GUIDE.md](https://github.com/Patm4nzzz/opentyrian-enhanced/blob/v1.0-rc1/docs/PLAYER-GUIDE.md).

## Licence

The code is GPL (see `COPYING`). The Tyrian 2000 data, and the images, music and sounds derived
from it, are freeware and **not** covered by the GPL; the MIDI arrangements and the SoundFont come
from third parties. See `DATA-NOTICES.txt` in the download.
