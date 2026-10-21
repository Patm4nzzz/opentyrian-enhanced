# ADR 0010 — Music: offline OPL rendering, music cues kept out of the simulated state

Date: 2026-10-04 — status: accepted (decision taken alone, "music" batch).

## Context
ADR 0004 chose to render `music.mus` offline to OGG using the existing C code. What remained to settle: which emulator,
how to loop, how a level triggers the music without touching determinism (ADR 0003), and what cannot be
reproduced exactly.

## Decisions
1. **Tool** `rewrite/tools/music-render/`: `music_render.c` compiles `src/lds_play.c`, `src/opl*.c`, `src/nuked_opl3.c`
   as-is (stubs.c provides `fread_die`, `ftell_eof`, the logs) and reproduces the OPL loop of
   `loudness.c:audioCallback` (`lds_update` at 69.5 Hz, `ldsUpdate2Rate = 139`, mono 16-bit 44,100 Hz PCM).
   Default emulator is **DOSBox**, like the C version (`opl.c:g_opl_backend`, `save_json.c:load_audio`).
   `render.sh` produces `song_00.ogg` … `song_40.ogg` (Vorbis q6) and `music.json` (title from `musmast.c:musicTitle`,
   duration, loop, end frame). `scripts/build-linux.sh` calls it (step 3b).
2. **Gain not applied** to the file: raw emulator output; the client applies the C version's gain
   (`volumeFactorTable[v] × 2`, 30 dB range) in dB.
3. **Looping**: the song is rendered until the 3rd visit to the same LDS player state at the start of a line
   (posplay, tempo, volumes); `loop_start` = 2nd visit, end = 3rd visit, with a 40 ms crossfade at the seam. Godot loops
   `AudioStreamOggVorbis` on `loop_offset`. A song without a loop (command 0xfc) stops; `end_frame` gives the
   frame where `playing` (lds_play.c) becomes false.
4. **Music cues in the simulation, outside the fingerprint**: `tyrian-sim` reproduces the C variables that decide the
   music (`musicFade`, `tempVolume`, `tyrMusicVolume`, events 34/35, game over, the end-of-level `play_song(9)`)
   and pushes `MusicCue` entries into a queue that the client drains (`take_music_cues`). These fields are not in
   `Sim::snapshot` nor in the golden master hashes; a test (`tyrian-golden/tests/music.rs`) replays a golden
   master with and without draining the queue and checks that the state is equal at every tick.
5. **Player-dependent branch**: `tyrian2.c` "else if (!playing && firstGameOver) play_song(levelSong - 1)"
   reads the state of the audio player. The simulation only exposes the simulated half (`music_level_song_branch`); the client
   tests `playing` (`Music.is_playing()`, based on `end_frame`) and restarts the level's song.
6. **Fades**: `fade_song()` (lds_fade(1)) acts inside the LDS player on the OPL registers, which cannot be reproduced on an
   already-rendered OGG. The client approximates it with a ramp of −11.5 dB/s cut off at 255/69.5 = 3.67 s, measured on the tool's
   render (`verify.py fade`, songs 29 and 17: below −40 dB at 3.5–3.6 s). `fade_and_play_song` (step of 8 every
   20 ms, `loudness.c:music_transition_update`) is reproduced as-is on the volume.

## Consequences
- The tool's render is **sample-identical** to the real audio output of the C version (SDL "disk" capture of the title
  screen, `capture_c_title.sh` + `verify.py compare`) once the C gain is applied.
- Accepted gaps: (a) the loop replays the 2nd pass instead of continuing the emulation (correlation 0.83 to 0.999 between
  two passes over 10 s: the DOSBox emulator keeps its phases and its `rand()` noise); (b) the `fade_song` fade is an
  approximation; (c) the Nuked emulator (a C option) is not rendered (`MUSIC_BACKEND=nuked` allows it); (d) the C version's
  MIDI/FluidSynth engine is not carried over (ADR 0004).
