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
	OPL3_WriteReg(&s_chip, (uint16_t)(idx & 0x1ff), val);
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
