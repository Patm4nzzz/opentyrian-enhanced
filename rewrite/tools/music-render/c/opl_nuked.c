/*
 *  opl_nuked.c — Nuked-OPL3 backend implementation.
 *
 *  Bit-exact Yamaha YMF262 emulator (Alexey Khokholov, 2013-2020).
 *  Exposes adlib_nuked_* functions; the public adlib_* ABI in opl.h
 *  is provided by opl.c's dispatcher that routes to either this
 *  backend or adlib_dosbox_* in opl_dosbox.c.
 */
#include "opl.h"
#include "nuked_opl3.h"

#include <string.h>

static opl3_chip s_chip;

void adlib_nuked_init(Bit32u samplerate)
{
	OPL3_Reset(&s_chip, samplerate);
}

void adlib_nuked_write(Bitu idx, Bit8u val)
{
	/* Buffered write: Nuked applies each queued write at least two chip
	 * samples after the previous one, as the port delays of the DOS
	 * player did on a real card.
	 *
	 * lds_play.c writes "key off" then "key on" for the same channel in a
	 * row (lds_playsound), with no sample generated in between. With the
	 * immediate OPL3_WriteReg the envelope generator never saw the key
	 * released, so a note played while the previous one was still held
	 * was not restarted: no attack, and an instrument that had decayed
	 * to silence stayed silent (docs/divergences.md, entry 66). */
	OPL3_WriteRegBuffered(&s_chip, (uint16_t)(idx & 0x1ff), val);
}

void adlib_nuked_getsample(Bit16s *sndptr, Bits numsamples)
{
	/* Nuked outputs stereo int16 pairs; Tyrian consumes a mono buffer
	 * and duplicates to stereo one level up in loudness.c. Mix the two
	 * channels to mono by signed arithmetic shift (matches the
	 * previous DOSBox backend's peak level). */
	for (Bits i = 0; i < numsamples; ++i)
	{
		Bit16s buf[2];
		OPL3_GenerateResampled(&s_chip, buf);
		sndptr[i] = (Bit16s)(((int32_t)buf[0] + (int32_t)buf[1]) >> 1);
	}
}
