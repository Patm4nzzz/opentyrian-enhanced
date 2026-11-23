# OpenTyrian2000 Enhanced v1.0-rc2 — second release candidate

**Tyrian 2000**, the 1995 vertical shoot 'em up, on a new engine: Rust runs the game, Godot 4
draws it, plays the sound and reads your keyboard, mouse and gamepad.

This second **release candidate** fixes the bugs players reported on v1.0-rc1, and the ones a
full test round found after them, with the settings a player really uses (Freeze Frames on, each
of the three music engines, mods turned on). Thank you for the reports: please keep them coming
on the repository's issue tracker.

## Downloads

Pick **one** game download for your system. The full downloads contain everything; the lite
downloads leave out the HD (AI) art, which you can add later with the HD pack.

| File | Size | What it is |
|---|---:|---|
| `OpenTyrian2000-Enhanced-v1.0-rc2-windows-x86_64.zip` | 708.9 MB | Windows, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc2-linux-x86_64.tar.gz` | 688.7 MB | Linux / Steam Deck, full (with HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc2-macos-universal.zip` | 732.2 MB | macOS (Intel and Apple Silicon), full, experimental |
| `OpenTyrian2000-Enhanced-v1.0-rc2-windows-x86_64-lite.zip` | 201.7 MB | Windows, lite (no HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc2-linux-x86_64-lite.tar.gz` | 190.1 MB | Linux / Steam Deck, lite (no HD (AI) art) |
| `OpenTyrian2000-Enhanced-v1.0-rc2-hd-pack.zip` | 506.2 MB | HD (AI) art pack for the lite downloads (Windows and Linux) |

Sizes in MB (1 MB = 1,000,000 bytes). The SHA-256 checksums are listed on the release page.

- **Windows**: extract the `.zip`, open the `OpenTyrian2000-Enhanced` folder, run
  `OpenTyrian2000-Enhanced.exe`.
- **Linux / Steam Deck**: extract the `.tar.gz`
  (`tar -xzf OpenTyrian2000-Enhanced-v1.0-rc2-linux-x86_64.tar.gz`), then run
  `./OpenTyrian2000-Enhanced.x86_64` from the `OpenTyrian2000-Enhanced` folder.
  `./install-desktop-entry.sh` adds a launcher with the Tyrian icon to your applications menu.
- **HD pack**: only for the lite downloads. Extract it **into** the game folder, so that
  `upscaled/` sits next to the executable, then pick Setup > Graphics > Sprites > Remastered (or
  F10, Graphics: HD (AI)). Use the pack of the same version as the game.

**Coming from v1.0-rc1** (Windows, Linux): extract v1.0-rc2 into a new folder, then copy the
`userdata` folder of your v1.0-rc1 game folder into it. Your settings, saves, scores, replays and
mods are in that folder. On macOS your files live outside the app: replace the app, nothing to copy.

## Fixed since v1.0-rc1

### Levels
- **A level no longer ends with its boss alive.** With Freeze Frames on (the default), destroying
  a large enemy while the scrolling was stopped for a boss could end the level about a second
  later: in DELIANI (episode 1), shooting one of the three ships that escort the boss was enough.
- **Ground enemies stay on their scenery.** With Freeze Frames on, turrets, buildings and wrecks
  slid a few pixels away from the ground they stand on at each freeze, up to 24 pixels in a level.
- **A special weapon picked up in a level is kept** after the level, in the shop and in the next
  level (the Ice Beam of DELIANI was lost).
- **The rear weapon mode is kept**: the mode chosen in the shop (Upgrade Ship > Rear Gun, "/") is
  the one the level starts with, and a mode switched during a level stays for the next one.
- **Two-player games: player 2 keeps its equipment** from one level to the next (rear weapon,
  sidekicks picked up and their upgrades); it went back to the starting equipment at each level.

### Music
- **Enhanced engine**: "Asteroid Dance, Part 1" and "Asteroid Dance, Part 2" each play their own
  piece, in the Jukebox and in the levels; the two were swapped.
- **Authentic engine**: notes played again while the previous one was still held are restarted.
  Whole voices were missing on many tracks ("Sarah's Song" played at about half its level); the
  41 tracks are rendered again and now match the Classic engine's level.

### Mods
- **Shop**: a mod ship shows its armor on the armor bar (it showed none), the shots of a mod
  weapon are drawn in the shop's preview, and the icons of mod weapons, shields and sidekicks
  show, as does a PNG that replaces the icon of an original item.
- **Every image folder of MODDING.md is drawn by the game**: enemies, coins and explosions in a
  level, the special weapon icon of the level HUD, the weapon pictures of the shop's ship
  illustration, the faces of the data cubes, and the planets of the navigation map with the title
  logos. Before, only ships, shots, sidekicks and shop icons were.
- **The mod check says more**: an icon whose PNG sits in the wrong `sprites/` folder is reported
  with the folder the shop reads, instead of showing nothing in silence. A PNG of `planets/` or
  `weapon_icons/` is named by the number of the sprite it replaces.
- **MODDING.md** says which folder is drawn where, and that a special weapon is given by a level
  event (`give_special`), not by `starter`.

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
- **macOS** is experimental: the app is not signed by Apple, see the player guide for the first
  launch.
- **Steam Deck**: not tested on real hardware yet.
- **HD pack version**: nothing stops you from installing an HD pack from another version; use the
  pack of the same version as the game.
- **No update notice in the game**: the game does not tell you when a new version is out. The
  current version is always at <https://tyrian-enhanced.com/download/>.

Full player guide: [docs/PLAYER-GUIDE.md](https://github.com/Patm4nzzz/opentyrian-enhanced/blob/v1.0-rc2/docs/PLAYER-GUIDE.md).
Everything v1.0-rc1 brought: [release notes of v1.0-rc1](https://github.com/Patm4nzzz/opentyrian-enhanced/releases/tag/v1.0-rc1).

## Licence

The code is GPL (see `COPYING`). The Tyrian 2000 data, and the images, music and sounds derived
from it, are freeware and **not** covered by the GPL; the MIDI arrangements and the SoundFont come
from third parties. See `DATA-NOTICES.txt` in the download.
