/*
 *  opl.c — dispatcher picking between two OPL FM emulators at runtime.
 *
 *  Backends:
 *    - DOSBox OPL (opl_dosbox.c): 2002-2010 ADLIBEMU port, traditional
 *      voicing, retained for users who prefer its sound or for
 *      comparison.
 *    - Nuked-OPL3 (opl_nuked.c): bit-exact Yamaha YMF262 (2013-2020),
 *      decapped-silicon accurate. Default backend.
 *
 *  Selection: g_opl_backend global, persisted in the SETT section of
 *  opentyrian.dat under audio.opl_backend. Settable at runtime via
 *  opl_set_backend() — re-init the current track after switching.
 *
 *  loudness.c and lds_play.c call the public adlib_* ABI below; they
 *  never need to know which backend is live.
 */
#include "opl.h"

#include <stdint.h>

/* DOSBox is the historical OpenTyrian OPL engine (shipped since 2007)
 * — the voicing every long-time player associates with Tyrian's music.
 * Nuked-OPL3 is technically more accurate to an actual YM3812 chip,
 * but "accurate to the chip" ≠ "accurate to player memory", so the
 * familiar sound wins as the default. Users who prefer the bit-exact
 * silicon can flip the switch in Setup > Sound > OPL Engine. */
opl_backend_t g_opl_backend = OPL_BACKEND_DOSBOX;

/* Forward declarations of the two backends' implementations. */
void adlib_dosbox_init(Bit32u samplerate);
void adlib_dosbox_write(Bitu idx, Bit8u val);
void adlib_dosbox_getsample(Bit16s *sndptr, Bits numsamples);
Bitu adlib_dosbox_reg_read(Bitu port);
void adlib_dosbox_write_index(Bitu port, Bit8u val);

void adlib_nuked_init(Bit32u samplerate);
void adlib_nuked_write(Bitu idx, Bit8u val);
void adlib_nuked_getsample(Bit16s *sndptr, Bits numsamples);

void opl_set_backend(opl_backend_t b)
{
	if (b == OPL_BACKEND_DOSBOX || b == OPL_BACKEND_NUKED)
		g_opl_backend = b;
}

opl_backend_t opl_get_backend(void)
{
	return g_opl_backend;
}

void adlib_init(Bit32u samplerate)
{
	if (g_opl_backend == OPL_BACKEND_DOSBOX)
		adlib_dosbox_init(samplerate);
	else
		adlib_nuked_init(samplerate);
}

void adlib_write(Bitu idx, Bit8u val)
{
	if (g_opl_backend == OPL_BACKEND_DOSBOX)
		adlib_dosbox_write(idx, val);
	else
		adlib_nuked_write(idx, val);
}

void adlib_getsample(Bit16s *sndptr, Bits numsamples)
{
	if (g_opl_backend == OPL_BACKEND_DOSBOX)
		adlib_dosbox_getsample(sndptr, numsamples);
	else
		adlib_nuked_getsample(sndptr, numsamples);
}

/* DOSBox-interface compat: no Tyrian caller uses these, but they're
 * declared in opl.h so we keep them wired. Only the DOSBox backend
 * ever bothered to implement them — Nuked doesn't model register
 * readback. Fall back to DOSBox for these regardless of the active
 * audio backend so any future caller gets something meaningful. */
Bitu adlib_reg_read(Bitu port)
{
	return adlib_dosbox_reg_read(port);
}

void adlib_write_index(Bitu port, Bit8u val)
{
	adlib_dosbox_write_index(port, val);
}
