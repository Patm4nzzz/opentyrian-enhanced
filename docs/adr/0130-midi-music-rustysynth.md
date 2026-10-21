# ADR 0130 — MIDI music: rustysynth in the extension, Original OPL / MIDI choice

Status: accepted (2026-10-05, card t_f20c39df, point 8). Replaces the "MIDI ruled out" part of
ADR 0004; the offline OPL rendering (ADR 0004, 0010) remains the default engine.

## Context
The C version plays the soundtrack either through OPL (DOSBox / Nuked) or General MIDI: "Enhanced"
engine (`midi_music.c`, FluidSynth loaded at runtime by `fluidsynth_dyn.c`), song
`n` (0-based) = first file `data/midi/NN-*.mid`, SoundFont `data/soundfont.sf2` (TimGM6mb),
infinite loop except jingles 9, 10 and 30, per-song OPL fallback if the `.mid` is missing
(`loudness.c:play_song`, `midi_fallback`). the maintainer asks for the same choice in the rewrite, through
an existing, maintained solution (no home-made synth), with the MIDI files and the SoundFont in
the packages (decision: the 41 MIDI files and the SoundFont stay in the repository).

## Options
1. **rustysynth** (Rust crate, MIT, SoundFont synth + MIDI sequencer, port of MeltySynth;
   1.3.7, no dependencies) in the `tyrian-godot` extension, rendering pulled by a Godot
   `AudioStreamGenerator`.
2. Existing MIDI GDExtension (godot-midi-player, etc.): none is published for Godot 4.7 with
   godot-rust; would add a second native binary per platform.
3. FluidSynth dynamically linked like the C version: system library absent on Windows and on Steam
   Deck, LGPL license to ship, C calls to port.

## Decision
Option 1.
- `tyrian-godot/src/midi.rs`: `MidiMusic` (rustysynth) and the `TyrianMidi` class as seen from GDScript
  (`load_soundfont`, `song_file`, `play_file`, `render`, `is_done`). 44.1 kHz, chorus and reverb
  enabled (like `midi_music.c`), 256 voices (rustysynth maximum), fixed gain 0.35 measured on the 41
  songs (highest peak 0.76, test `loudest_tracks_do_not_clip`).
- **Loop**: where the C version replays the file from the beginning, the rewrite resumes at the same instant
  of the song as the OPL rendering (`music.json`: `loop_start − (duration − loop_start)`), converted into a
  tick through the tempo map (crate `midly`, public domain) and passed to rustysynth
  (`MidiFileLoopType::LoopPoint`). Songs without a loop in `music.json` (9, 10, 18, 24, 30, 33)
  play once; `Music.is_playing` then follows `TyrianMidi.is_done`, which keeps the restart of the
  level song (`level.gd`) common to both engines.
- **Song changes**: unchanged, `music.gd` keeps `play_song` / `fade_song` /
  `fade_and_play_song` / `set_volume`; only `_load` chooses the player. Volume: same table as OPL,
  without its doubling (`loudness.c`: `musicVolumeFactor *= 2` reserved for OPL).
- **Fallback**: SoundFont missing or unreadable, or song without a readable `.mid` → the OPL version of that
  song plays, with a warning (`push_warning`), never silence; the player's choice
  remains MIDI (like the C `music_backend_preference`).
- **Setting**: `music_engine` (0 Original, 1 MIDI; default 0) in `user://setup.cfg`, row
  "Music Engine" of the Sound menu (PICKER of the menu engine). The C "Authentic" choice (live Nuked)
  does not exist: the OPL music is an offline rendering.
- **Packages**: `package.sh:copy_data` copies `data/soundfont.sf2` and `data/midi/NN-*.mid`;
  `DATA-NOTICES.txt` says that their third-party licenses are not included.

## Consequences
- Simulation and golden masters intact: audio stays outside `tyrian-sim`.
- Testable without an audio device: in-memory rendering in `cargo test -p tyrian-godot midi`.
- The sequencer runs on the main thread (`_process`, 0.25 s reserve); a frame longer than
  0.25 s can cause a gap, like any frame-driven playback.
- The timbre differs from FluidSynth (another engine on the same SoundFont); this is not parity.
