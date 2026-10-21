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
#include "musmast.h"

#include "opentyr.h"
#include <stdint.h>
#include <stdbool.h>

uint8_t songBuy;

const char musicTitle[MUSIC_NUM][48] =
{
	"Asteroid Dance, Part 2",      /*  0 */
	"Asteroid Dance, Part 1",      /*  1 */
	"Buy/Sell Screen",             /*  2 */
	"Camanis",                     /*  3 */
	"Camanis Boss",                /*  4 */
	"Deliani",                     /*  5 */
	"Deliani Alternate",           /*  6 */
	"Ending 1",                    /*  7 */
	"Ending 2",                    /*  8 */
	"End of Level",                /*  9 */
	"Game Over",                   /* 10 */
	"Gryphon",                     /* 11 */
	"Gryphon Boss",                /* 12 */
	"Gyges",                       /* 13 */
	"Gyges Boss",                  /* 14 */
	"Hallows",                     /* 15 */
	"Tunnel",                      /* 16 */
	"Tyrian, the Level",           /* 17 */
	"The Musicman",                /* 18 */
	"The Navigator",               /* 19 */
	"Savara",                      /* 20 */
	"Savara Boss",                 /* 21 */
	"Space Battle 1",              /* 22 */
	"Space Battle 2",              /* 23 */
	"Ending 3",                    /* 24 */
	"Secret Level",                /* 25 */
	"Story Mode",                  /* 26 */
	"Torm",                        /* 27 */
	"Transon Lohk",                /* 28 */
	"Tyrian - Main Title",         /* 29 */
	"Power Up",                    /* 30 */
	"Secret Level 2",              /* 31 */
	"Savara, the Return",          /* 32 */
	"High Scores",                 /* 33 */
	"One Must Fall Tribute",       /* 34 */
	"Sarah's Song",                /* 35 */
	"A Field for Mag",             /* 36 */
	"Rock Garden",                 /* 37 */
	"Quest for Peace",             /* 38 */
	"Composition in Q",            /* 39 */
	"Beer",                        /* 40 */
};

bool musicFade;
