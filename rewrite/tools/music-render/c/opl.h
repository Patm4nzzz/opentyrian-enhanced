/*
 *  Copyright (C) 2002-2010  The DOSBox Team
 *  OPL2/OPL3 emulation library
 *
 *  This library is free software; you can redistribute it and/or
 *  modify it under the terms of the GNU Lesser General Public
 *  License as published by the Free Software Foundation; either
 *  version 2.1 of the License, or (at your option) any later version.
 * 
 *  This library is distributed in the hope that it will be useful,
 *  but WITHOUT ANY WARRANTY; without even the implied warranty of
 *  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 *  Lesser General Public License for more details.
 * 
 *  You should have received a copy of the GNU Lesser General Public
 *  License along with this library; if not, write to the Free Software
 *  Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA
 */

#ifndef OPL_H
#define OPL_H
/*
 * Originally based on ADLIBEMU.C, an AdLib/OPL2 emulation library by Ken Silverman
 * Copyright (C) 1998-2001 Ken Silverman
 * Ken Silverman's official web site: "http://www.advsys.net/ken"
 */

#include <stdint.h>

typedef uintptr_t	Bitu;
typedef intptr_t	Bits;
typedef uint32_t	Bit32u;
typedef int32_t		Bit32s;
typedef uint16_t	Bit16u;
typedef int16_t		Bit16s;
typedef uint8_t		Bit8u;
typedef int8_t		Bit8s;

/* general functions — dispatch at runtime to the selected backend */
void adlib_init(Bit32u samplerate);
void adlib_write(Bitu idx, Bit8u val);
void adlib_getsample(Bit16s* sndptr, Bits numsamples);

Bitu adlib_reg_read(Bitu port);
void adlib_write_index(Bitu port, Bit8u val);

/* Runtime backend selection — setters are re-safe during playback
 * (the next adlib_init call picks up the new choice). */
typedef enum {
	OPL_BACKEND_DOSBOX = 0,  /* ADLIBEMU 2002-2010 */
	OPL_BACKEND_NUKED  = 1,  /* Nuked-OPL3 bit-exact 2013-2020 */
} opl_backend_t;

extern opl_backend_t g_opl_backend;

void          opl_set_backend(opl_backend_t b);
opl_backend_t opl_get_backend(void);

#define opl_init() adlib_init(audioSampleRate)
#define opl_write(reg, val) adlib_write(reg, val)
#define opl_update(buf, num) adlib_getsample(buf, num)

#endif /* OPL_H */
