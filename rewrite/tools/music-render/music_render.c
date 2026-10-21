/*
 * music_render: offline rendering of the songs of music.mus (ADR 0004, 0010).
 *
 * Compiles as-is src/lds_play.c, src/opl.c, src/opl_dosbox.c,
 * src/opl_nuked.c and src/nuked_opl3.c (no change to the C version);
 * stubs.c provides the few game symbols they reference.
 *
 * The rendering loop reproduces loudness.c:audioCallback (OPL branch):
 *   - lds_update() every samplesPerLdsUpdate (+ fraction) frames, with
 *     ldsUpdate2Rate = 139 (69.5 Hz);
 *   - opl_update() produces a 16-bit mono stream (the C duplicates it to stereo).
 * The final C gain (volumeFactorTable x 2, loudness.c) is NOT applied
 * here: the raw emulator output is written, the Godot client applies the
 * gain (see rewrite/godot/scripts/music.gd).
 *
 * Loop: the LDS player returns to the start of a pattern row (pattplay == 0,
 * tempo_now == 0) in a state (posplay, tempo, volume) already met as soon as
 * the song loops (command 0xf9, backward jump, or return to position 0 by
 * maxpos overflow). Rendering goes on until the THIRD visit of the same state:
 * [0, 2nd visit) = intro + one pass, [2nd, 3rd) = one full pass; the output
 * stops at the 3rd visit and loop_start = 2nd visit. This way the loop
 * point falls after a full pass, when the OPL registers (ringing
 * notes) are in the same state as at the next loop.
 * Song without a loop (command 0xfc -> playing = false): rendered until
 * it dies out (or TAIL_MAX_S), no loop.
 *
 * Splice: the DOSBox emulator keeps its phase accumulators and its noise
 * (rand()) from one pass to the next, so [loop_end, ...) is not sample-for-sample
 * identical to [loop_start, ...) (correlation 0.83 to 0.999 over
 * 10 s, see verify.py loops). So that the loop_end -> loop_start jump in Godot
 * does not click, X = LOOP_XFADE_MS more is rendered after loop_end, this real
 * continuation is faded towards [loop_start, loop_start + X), then both
 * points are shifted by X: the end of the file is then exactly the sample
 * preceding the new loop_start. Pass duration unchanged.
 *
 * Usage: music_render <music.mus> <0-based index> <output.pcm> [nuked|dosbox] [rate] [extra_s] [fade_at_ms]
 * extra_s (verification only): keeps rendering extra_s seconds after
 * the loop end, to compare [loop_end, +extra) with [loop_start, +extra).
 * fade_at_ms (measurement only, verify.py fade): calls lds_fade(1) -- what
 * loudness.c:fade_song does -- at the first LDS update after fade_at_ms,
 * renders 5 more seconds and stops, without searching for a loop.
 * Standard output: one line of JSON metadata.
 */
#include "opentyr.h"
#include "lds_play.h"
#include "loudness.h"
#include "opl.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* loudness.c */
static const int ldsUpdate2Rate = 139;

#define MAX_SECONDS 900   /* safeguard: no song lasts 15 min */
#define TAIL_MAX_S 8      /* tail after 0xfc */
#define MAX_KEYS 4096
/* Splice crossfade at the loop point, in ms (see below). */
#define LOOP_XFADE_MS 40

/* Internal state of lds_play.c: static in that file, so it is included in
 * this translation unit to be able to read posplay / pattplay / tempo. */
#include "lds_play.c"

typedef struct {
	Uint16 posplay;
	Uint8 tempo, allvolume, fadeonoff;
	Uint16 mainvolume;
	int visits;
	long first, second;
	int first_update;
	int second_update;
} Key;

static Key keys[MAX_KEYS];
static int key_count = 0;

static Key *find_key(void)
{
	for (int i = 0; i < key_count; i++)
	{
		Key *k = &keys[i];
		if (k->posplay == posplay && k->tempo == tempo && k->allvolume == allvolume &&
		    k->fadeonoff == fadeonoff && k->mainvolume == mainvolume)
			return k;
	}
	if (key_count == MAX_KEYS)
		return NULL;
	Key *k = &keys[key_count++];
	memset(k, 0, sizeof *k);
	k->posplay = posplay;
	k->tempo = tempo;
	k->allvolume = allvolume;
	k->fadeonoff = fadeonoff;
	k->mainvolume = mainvolume;
	return k;
}

int main(int argc, char **argv)
{
	if (argc < 4)
	{
		fprintf(stderr, "usage: %s music.mus index out.pcm [nuked|dosbox] [rate]\n", argv[0]);
		return 2;
	}
	const char *mus_path = argv[1];
	int index = atoi(argv[2]);
	const char *out_path = argv[3];
	const char *backend = argc > 4 ? argv[4] : "dosbox";
	int rate = argc > 5 ? atoi(argv[5]) : 44100;
	int extra_s = argc > 6 ? atoi(argv[6]) : 0;
	long fade_at = argc > 7 ? (long)rate * atol(argv[7]) / 1000 : -1;
	long fade_started = -1;

	opl_set_backend(strcmp(backend, "dosbox") == 0 ? OPL_BACKEND_DOSBOX : OPL_BACKEND_NUKED);
	audioSampleRate = rate;

	/* loudness.c:load_music */
	FILE *f = fopen(mus_path, "rb");
	if (f == NULL)
	{
		perror(mus_path);
		return 1;
	}
	Uint16 song_count;
	fread_u16_die(&song_count, 1, f);
	if (index < 0 || index >= song_count)
	{
		fprintf(stderr, "index %d out of 0..%d\n", index, song_count - 1);
		return 1;
	}
	Uint32 *song_offset = malloc((song_count + 1) * sizeof *song_offset);
	fread_u32_die(song_offset, song_count, f);
	song_offset[song_count] = (Uint32)ftell_eof(f);

	/* loudness.c:load_song -> lds_load (which calls lds_rewind -> opl_init) */
	if (!lds_load(f, song_offset[index], song_offset[index + 1] - song_offset[index]))
		return 1;

	FILE *out = fopen(out_path, "wb");
	if (out == NULL)
	{
		perror(out_path);
		return 1;
	}

	/* loudness.c:init_audio */
	const int samplesPerLdsUpdate = 2 * (rate / ldsUpdate2Rate);
	const int samplesPerLdsUpdateFrac = 2 * (rate % ldsUpdate2Rate);
	int samplesUntilLdsUpdate = 0;
	int samplesUntilLdsUpdateFrac = 0;

	static Sint16 buf[4096];
	size_t pcm_cap = (size_t)rate * 60;
	Sint16 *pcm = malloc(pcm_cap * sizeof *pcm);
	const long xfade = (long)rate * LOOP_XFADE_MS / 1000;
	long written = 0;
	long loop_start = -1, loop_end = -1;
	int updates = 0, loop_start_update = -1, loop_end_update = -1;
	long stop_at = fade_at >= 0 ? fade_at + (long)rate * 5 : (long)rate * MAX_SECONDS;
	long ended_at = -1;     /* frame where playing became false */
	long quiet_run = 0;
	Sint32 peak = 0;
	double sum_sq = 0;

	while (written < stop_at)
	{
		if (samplesUntilLdsUpdate == 0)
		{
			/* Start of a pattern row: possible loop point. */
			if (fade_at >= 0 && fade_started < 0 && written >= fade_at)
			{
				lds_fade(1);
				fade_started = written;
			}
			if (fade_at < 0 && playing && tempo_now == 0 && pattplay == 0 && loop_end < 0)
			{
				Key *k = find_key();
				if (k != NULL)
				{
					k->visits++;
					if (k->visits == 1) { k->first = written; k->first_update = updates; }
					else if (k->visits == 2) { k->second = written; k->second_update = updates; }
					else
					{
						loop_start = k->second;
						loop_start_update = k->second_update;
						loop_end = written;
						loop_end_update = updates;
						stop_at = written + (extra_s > 0 ? (long)rate * extra_s : xfade);
					}
				}
			}

			bool was_playing = playing;
			lds_update();
			updates++;
			if (was_playing && !playing && ended_at < 0)
			{
				ended_at = written;
				stop_at = written + (long)rate * TAIL_MAX_S;
			}

			samplesUntilLdsUpdate += samplesPerLdsUpdate;
			samplesUntilLdsUpdateFrac += samplesPerLdsUpdateFrac;
			if (samplesUntilLdsUpdateFrac >= ldsUpdate2Rate)
			{
				samplesUntilLdsUpdate += 1;
				samplesUntilLdsUpdateFrac -= ldsUpdate2Rate;
			}
		}

		int chunk = MIN(samplesUntilLdsUpdate, 4096);
		if (written + chunk > stop_at)
			chunk = (int)(stop_at - written);
		opl_update(buf, chunk);
		if ((size_t)(written + chunk) > pcm_cap)
		{
			pcm_cap *= 2;
			pcm = realloc(pcm, pcm_cap * sizeof *pcm);
		}
		memcpy(pcm + written, buf, (size_t)chunk * sizeof *buf);
		for (int i = 0; i < chunk; i++)
		{
			Sint32 a = buf[i] < 0 ? -buf[i] : buf[i];
			if (a > peak) peak = a;
			sum_sq += (double)buf[i] * buf[i];
			quiet_run = (a < 8) ? quiet_run + 1 : 0;
		}
		written += chunk;
		samplesUntilLdsUpdate -= chunk;

		/* After 0xfc: cut as soon as there is half a second of silence. */
		if (fade_at < 0 && ended_at >= 0 && quiet_run > rate / 2)
			break;
	}
	if (loop_end >= 0 && extra_s <= 0 && written == loop_end + xfade)
	{
		for (long i = 0; i < xfade; i++)
		{
			double w = (double)(i + 1) / xfade;
			double v = (1.0 - w) * pcm[loop_end + i] + w * pcm[loop_start + i];
			pcm[loop_end + i] = (Sint16)lround(v);
		}
		loop_start += xfade;
		loop_end += xfade;
	}
	fwrite(pcm, sizeof *pcm, (size_t)written, out);
	free(pcm);
	fclose(out);
	fclose(f);

	const char *status = loop_end >= 0 ? "loop" : (ended_at >= 0 ? "end" : "cap");
	printf("{\"index\":%d,\"backend\":\"%s\",\"rate\":%d,\"frames\":%ld,"
	       "\"loop_start\":%ld,\"loop_end\":%ld,\"loop_start_update\":%d,\"loop_end_update\":%d,"
	       "\"updates\":%d,\"ended_at\":%ld,\"status\":\"%s\",\"peak\":%d,\"rms\":%.1f,"
	       "\"allvolume\":%d,\"mainvolume\":%d,\"fade_started\":%ld}\n",
	       index, backend, rate, written, loop_start, loop_end, loop_start_update, loop_end_update,
	       updates, ended_at, status, peak, written ? sqrt(sum_sq / written) : 0.0,
	       allvolume, mainvolume, fade_started);
	return 0;
}
