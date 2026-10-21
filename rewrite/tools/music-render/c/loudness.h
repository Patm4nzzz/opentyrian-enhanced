/* 
 * OpenTyrian: A modern cross-platform port of Tyrian
 * Copyright (C) 2007-2009  The OpenTyrian Development Team
 *
 * This program is free software; you can redistribute it and/or
 * modify it under the terms of the GNU General Public License
 * as published by the Free Software Foundation; either version 2
 * of the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA.
 */
#ifndef LOUDNESS_H
#define LOUDNESS_H

#include "opentyr.h"
#include "opl.h"

#include "SDL.h"

typedef enum {
	MUSIC_OPL = 0,
	MUSIC_FLUIDSYNTH = 1,
} MusicBackend;

extern MusicBackend music_backend;
/* User's choice; music_backend may differ from it if
 * initialization had to fall back to OPL. This is the one that gets persisted. */
extern MusicBackend music_backend_preference;
extern bool stereo_enabled;
extern bool music_pseudo_stereo;

/* Audio-callback lock helpers. Main-thread code that writes any global
 * the callback reads (stereo_enabled, music_backend, …) must hold this
 * around the write. music_pseudo_stereo has its own locked setter that
 * also resets the widener delay ring on enable. */
void audio_lock(void);
void audio_unlock(void);
void music_pseudo_stereo_set(bool enabled);

extern int audioSampleRate;

/* Index of the track last handed to a player, or SONG_NONE. Index 0 is a
 * real track ("Asteroid Dance, Part 2"): 0 was used as the "nothing
 * playing" sentinel, which made that track unrestartable and confused
 * the jingle-restore logic. */
#define SONG_NONE ((unsigned int)-1)
extern unsigned int song_playing;

extern bool audio_disabled, music_disabled, samples_disabled;

bool init_audio(void);
void deinit_audio(void);

void load_music(void);
void play_song(unsigned int song_num);
void restart_song(void);
void stop_song(void);
void fade_song(void);

void set_volume(Uint8 musicVolume, Uint8 sampleVolume);
void fade_and_play_song(unsigned int song_num);
void music_transition_update(Uint32 frame_ms);

/* Refresh the LDS-style `playing` / `songlooped` flags from the MIDI
 * player when FluidSynth is in charge (OPL maintains them itself). The
 * jukebox polls this every frame so a one-shot jingle advances to the
 * next track and a looped track fades like it does on OPL. Not called
 * from the level loop on purpose: there, a finished game-over jingle is
 * meant to stay silent (see play_song). */
void music_poll(void);

void multiSamplePlay(const Sint16 *samples, size_t sampleCount, Uint8 chan, Uint8 vol);

/* Panned variant: pan = 0 (full left) to 255 (full right), 128 = center */
void multiSamplePlayPan(const Sint16 *samples, size_t sampleCount, Uint8 chan, Uint8 vol, Uint8 pan);

/* Pitched variant: pitch_step is a Q16.16 fixed-point sample step.
 * 0x10000 = native pitch (1.0x). Range typically [0x8000..0x20000]
 * for ±1 octave; ±5% randomization uses [~0xF333..~0x10CCC]. */
void multiSamplePlayPanPitch(const Sint16 *samples, size_t sampleCount, Uint8 chan, Uint8 vol, Uint8 pan, Uint32 pitch_step);

/* Feature toggles (persisted via save_json). */
extern bool sfx_pitch_variation; /* ±5% random pitch on every SFX */
extern bool sfx_impact_scaling;  /* deeper/louder explosions for large enemies + bosses */

/* Converts an x position in the PLAY AREA to a pan value (0-255).
 *
 * The formula divided by 320, the width of the whole screen. But the action
 * takes place between x=24 and x=288 (PLAY_AREA_X..+PLAY_AREA_WIDTH), and the
 * ship itself is bounded to [40, 256]: the pan therefore only covered
 * 31..204 instead of 0..255, and the sound image was biased to the left --
 * measured, +12.3 dB on the left against -8.0 dB on the right for a ship stuck
 * to each edge. It is now relative to the play area to recover the whole
 * stereo width. */
static inline Uint8 screen_x_to_pan(int x)
{
	int rel = x - 24;                 /* PLAY_AREA_X */
	if (rel < 0) rel = 0;
	if (rel > 264) rel = 264;         /* PLAY_AREA_WIDTH */
	return (Uint8)(rel * 255 / 264);
}

/* Drain soundQueue[]: play any queued SFX with per-channel pan, random
 * pitch jitter, and optional damage-scaled depth/volume. Clears the
 * queue slots after dispatching. No-op unless firstGameOver is set. */
void JE_flushSoundQueue(void);

/* Hard-stop every sample channel. Call when leaving gameplay (level
 * end, quit-to-title) so a voice sample mid-playback doesn't carry
 * over into the next screen. Drains soundQueue[] too so any pending
 * write doesn't fire on the next flushSoundQueue. */
void stop_all_samples(void);

#endif /* LOUDNESS_H */
