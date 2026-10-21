# ADR 0004 — Audio: SFX as WAV, OPL music rendered offline, MIDI set aside

Date: 2026-10-04 — status: accepted (decision made alone). The "MIDI set aside" part was replaced by
ADR 0130 (MIDI music at the player's choice, rustysynth).

## Context
Effects: `tyrian.snd` and `voices.snd`, 8-bit signed PCM at 11,025 Hz (`nortsong.c:loadSndFile`). Music: `music.mus`, 41 Loudness tracks for OPL2 (`loudness.c`, `lds_play.c`), played in C by emulation (DOSBox OPL, Nuked OPL3) or replaced by MIDI through FluidSynth. Godot has neither an OPL emulator nor a native MIDI synth.

## Options
1. OPL emulator in Rust inside the GDExtension, fed in real time → faithful but a big undertaking.
2. Offline OPL → OGG rendering with the existing C code (`lds_play.c` + `nuked_opl3.c`) compiled into a small separate tool, then played back through `AudioStreamPlayer`.
3. MIDI + SoundFont → unclear rights on the MIDI arrangements (`formats-spec.md` §13), and a FluidSynth dependency.

## Decision
Option 2 for the music, direct conversion to WAV for the effects. The MIDI files in `data/midi/` are not carried over.

## Consequences
The music rendering tool reuses GPL v2 code from the repository without modifying the C version. Until it is delivered, the Godot version plays the sound effects alone.
