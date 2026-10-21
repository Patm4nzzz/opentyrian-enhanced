OpenTyrian2000 Enhanced
=======================

An arcade-style shoot 'em up: Tyrian 2000, rebuilt on a Godot 4 + Rust engine.

Start the game
--------------
Windows : double-click OpenTyrian2000-Enhanced.exe
Linux   : ./OpenTyrian2000-Enhanced.x86_64   (chmod +x it first if needed)
          ./install-desktop-entry.sh adds a launcher with the Tyrian icon to
          your applications menu (optional, removable with --remove).

Keep this whole folder together: the game needs the .pck file, the engine
library (tyrian_godot.dll / libtyrian_godot.so), data/ and upscaled/ next to
the executable. Without upscaled/ the "HD (AI)" graphics mode is not offered.
The "lite" download has no upscaled/ folder: to add the HD (AI) mode, get the
HD pack of the same release (OpenTyrian2000-Enhanced-<version>-hd-pack.zip,
with the same version as this download, e.g. v1.0-rc1) and extract
it into this folder (see HD-PACK-README.txt in the pack).

Basic controls
--------------
Move: arrow keys      Fire: Space        Rear-gun mode: Enter
Sidekicks: Ctrl / Alt Pause: P           Help in game: F1
Controls and display settings: F9 / F10. A gamepad works too.

Command line
------------
Game options go after "--":  ./OpenTyrian2000-Enhanced.x86_64 -- --help
lists them (-s no sound, -t DIR data folder, --lang=fr, --hd-sprites, ...).
Engine options (--fullscreen, --resolution, --audio-driver, ...) go before "--".

Your old OpenTyrian2000 (C version) profile
-------------------------------------------
On the first launch the game reads the C version's opentyrian.dat once: in the
folder of the game (or its neighbour OpenTyrian2000 folder) on Windows,
~/.config/opentyrian2000 on Linux. It copies your settings, keys, high scores,
saves and achievements; the original file is never changed.

Where your files go
-------------------
Settings, saves, high scores, replays, screenshots and your mods are stored
in the userdata folder next to the game executable (created at first launch).
Copy the whole game folder to keep your game; delete userdata to start fresh.
If the game folder is read-only (e.g. Program Files), they go to the Godot
user folder instead:
  Windows : %APPDATA%\Godot\app_userdata\OpenTyrian2000 Enhanced
  Linux   : ~/.local/share/godot/app_userdata/OpenTyrian2000 Enhanced

What is in this folder
----------------------
COPYING            licence of the game code (GPL)
DATA-NOTICES.txt   licences and credits of the game data, music and HD art
licenses/          licence texts of the bundled library and SoundFont

More: https://github.com/Patm4nzzz/opentyrian-enhanced
