# macOS test checklist (experimental, unsigned build)

Thank you for testing! This is the first macOS build of OpenTyrian2000 Enhanced. Nobody on the team
has a Mac, so **everything you report is new information**, including "it just works".
Tick what you tried, note what went wrong, and send back the items of the last section.

- File: `OpenTyrian2000-Enhanced-v<version>-macos-universal.zip` (one app for Intel and Apple Silicon,
  HD (AI) art included, about 0.7 GB).
- Needs macOS 11 (Big Sur) or later.
- The app is **not** signed with an Apple Developer ID nor notarized: macOS will warn you the first time.
  That is expected.

## 1. Opening the app

- [ ] Double-click the `.zip`; `OpenTyrian2000 Enhanced.app` appears. Move it to Applications (optional).
- [ ] Double-click the app. Note **the exact warning** (screenshot): "cannot be opened because the
      developer cannot be verified", "is damaged and can't be opened", "Apple could not verify…"?
- [ ] Open it with **one** of these, and note which one worked:
  1. right-click (or Control-click) the app › **Open** › **Open**;
  2. **System Settings › Privacy & Security**, scroll down, **Open Anyway** (macOS 15 and later: the only
     graphical way);
  3. Terminal: `xattr -dr com.apple.quarantine "/Applications/OpenTyrian2000 Enhanced.app"`, then open it normally.
- [ ] The title screen appears, with music. Time from double-click to title screen: ______ s.
- [ ] The app icon (Tyrian ship) shows in the Dock and in Finder.

## 2. Display (Retina, windowed / fullscreen)

- [ ] Window mode: the picture is sharp (not blurry), the window has a sensible size on your screen.
- [ ] **Alt+Enter** (Option+Enter) toggles fullscreen and back; the green window button too.
- [ ] Fullscreen on a Retina screen: sharp picture, black bars only where expected, no flicker.
- [ ] Moving the window to a second screen (if you have one), then back.
- [ ] **F10** (on a MacBook: **fn+F10**) › Graphics: cycle **Original**, **Sharp pixel art** and **HD (AI)**.
      HD (AI) must be offered (the art is inside the app). Play a minute in each.
- [ ] CRT modes and Scaler in the same menu: any visual glitch?

## 3. Controls

- [ ] Keyboard: arrows move, **Space** fires, **Enter** / **Esc** in menus. Do the function keys need
      **fn** on your keyboard? (System Settings › Keyboard › "Use F1, F2, etc. keys as standard function keys".)
- [ ] **F9** controls screen: remap a key, it is kept after relaunch.
- [ ] Mouse: menus can be clicked; mouse steering in game (see the player guide §2.2).
- [ ] Gamepad (Xbox, PlayStation, Switch Pro, 8BitDo…): note the model and how it is connected
      (USB / Bluetooth). Does it move the ship, fire, open menus? **Setup › Joystick** sees it?
- [ ] Two players (keyboard + gamepad), if you can.

## 4. Sound and music

- [ ] Sound effects play (shots, explosions).
- [ ] **Setup › Sound**: try the **three music engines** (Classic, Authentic, MIDI); each one plays
      music, switching does not crash or go silent.
- [ ] Volumes change the sound; muting works.
- [ ] Unplug / plug headphones or switch the output in the menu bar while playing: does the sound follow?

## 5. Playing, saving, loading

- [ ] Start a new game (episode 1), play the first level to the end, visit the shop.
- [ ] Save in a slot, quit to the title, **Load Game**: the save is there and loads.
- [ ] Endless mode (title screen, **2**): play a run, the leaderboard keeps the score.
- [ ] Record a replay (see the player guide §3.1) and watch it on the **Replays** screen.
- [ ] Screenshot key in a level (guide §2.6): a `screenshot_0000.bmp` appears in the data folder (below).

## 6. Your files (data folder)

Everything the game writes goes to `~/Library/Application Support/OpenTyrian2000 Enhanced/`
(Finder: **Go › Go to Folder…**, paste the path). Nothing must be written inside the app.

- [ ] That folder exists after the first launch and contains `display.cfg`, `setup.cfg`, `saves/`,
      `replays/`, `mods/`, `logs/`.
- [ ] Right-click the app › **Show Package Contents** › `Contents/Resources`: contains `data/`, `mods/`,
      `upscaled/`. Do **not** change anything there (that breaks the signature).

## 7. Mods

- [ ] **Setup › Mods** lists the example mods (shipped inside the app); enable one, **Esc**, play:
      the change shows.
- [ ] **Install a mod (.zip)**: pick a mod `.zip` (or drop it on the window). It is copied to
      `~/Library/Application Support/OpenTyrian2000 Enhanced/mods/`.
- [ ] **Open mods folder** opens that folder in Finder.

## 8. Quit and relaunch

- [ ] Quit from the title menu, then with **Cmd+Q**, then with the window's red button. Each time the
      app closes cleanly (no "quit unexpectedly" report).
- [ ] Relaunch: settings (graphics mode, volumes, controls, fullscreen) are kept, and no Gatekeeper
      warning comes back.
- [ ] Optional: let the Mac sleep while the game is paused, wake it, resume.

## 9. Intel and Apple Silicon

The app contains both an Intel (x86_64) and an Apple Silicon (arm64) version.

- [ ] Note your Mac's chip:  › About This Mac (Chip: Apple M…, or Processor: Intel…).
- [ ] Activity Monitor › CPU tab › **Kind** column for "OpenTyrian2000 Enhanced": "Apple" on Apple
      Silicon, "Intel" on an Intel Mac.
- [ ] Apple Silicon, optional: Finder › app › **Get Info** › tick **Open using Rosetta**, launch, play a
      level (this checks the Intel version), then untick it.
- [ ] Smoothness: steady speed? Any stutter, fans spinning up, battery drain?

## 10. What to send back

Please send (issue on the repository, or message to the maintainer):

1. **macOS version** ( › About This Mac, e.g. "macOS 15.1 Sequoia") and **Mac model + chip**
   (e.g. "MacBook Air M2, 2022" or "iMac 27" Intel i5, 2019"), screen(s) used (built-in Retina, external).
2. The build file name you tested (`OpenTyrian2000-Enhanced-v…-macos-universal.zip`).
3. Which opening method of §1 worked, and **screenshots** of the warnings (Cmd+Shift+4, then
   Space and click a window, saves to the Desktop).
4. Screenshots of anything wrong (glitch, wrong colors, text cut off).
5. The **log file**: `~/Library/Application Support/OpenTyrian2000 Enhanced/logs/godot.log` (older
   runs: the other files of `logs/`). Send it right after a problem, before relaunching the game
   several times.
6. If the app crashed: the macOS crash report ("Report…" button, or **Console** app › Crash Reports ›
   `OpenTyrian2000 Enhanced…ips`).
7. Your gamepad model and connection, if you used one.
8. This list with the boxes ticked and short notes ("OK", "KO: …").
