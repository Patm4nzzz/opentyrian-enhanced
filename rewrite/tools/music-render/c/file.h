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
#ifndef FILE_H
#define FILE_H

#include "SDL_endian.h"

#include <stdbool.h>
#include <stdio.h>

extern const char *custom_data_dir;

const char *app_base_dir(void);
const char *data_dir(void);

FILE *dir_fopen(const char *dir, const char *file, const char *mode);
FILE *dir_fopen_warn(const char *dir, const char *file, const char *mode);
FILE *dir_fopen_die(const char *dir, const char *file, const char *mode);

/* fopen that accepts a UTF-8 path on Windows (the ANSI CRT broke
 * any accented path -- W1/W2); plain fopen(3) elsewhere. */
FILE *fopen_utf8(const char *path, const char *mode);
/* remove()/rename() that accept UTF-8 on Windows (W1); elsewhere, the
 * standard functions. rename_utf8 does NOT replace an existing target on
 * Windows -- do remove_utf8 first (or MoveFileExW for atomicity). */
int remove_utf8(const char *path);
int rename_utf8(const char *oldpath, const char *newpath);

/* Directory traversal that accepts UTF-8 on Windows (the ANSI CRT's opendir()
 * did not find an accented folder and returned names in the local code
 * page); plain opendir(3)/readdir(3) elsewhere.
 *
 *   OTDir *d = opendir_utf8(path);
 *   const char *name;
 *   while (d != NULL && (name = readdir_utf8(d)) != NULL) ...
 *   closedir_utf8(d);
 *
 * readdir_utf8 returns the entry name in UTF-8, valid until the next
 * call; NULL at the end of the traversal. closedir_utf8(NULL) has no effect. */
typedef struct OTDir OTDir;
OTDir *opendir_utf8(const char *path);
const char *readdir_utf8(OTDir *d);
void closedir_utf8(OTDir *d);

#ifdef TARGET_WIN32
#include <wchar.h>
/* UTF-8 -> UTF-16. Allocated: to be freed by the caller. NULL on failure. */
wchar_t *utf8_to_wide_alloc(const char *utf8);
#endif

bool dir_file_exists(const char *dir, const char *file);

/* Creates a directory and all its parents. Idempotent. */
void dir_make_recursive(const char *path);

long ftell_eof(FILE *f);

void fread_die(void *buffer, size_t size, size_t count, FILE *stream);

// 8-bit fread that dies if read fails
static inline void fread_bool_die(bool *buffer, FILE *stream)
{
	Uint8 temp;
	fread_die(&temp, sizeof(Uint8), 1, stream);
	*buffer = temp != 0;
}

// 8-bit fread
static inline size_t fread_u8(Uint8 *buffer, size_t count, FILE *stream)
{
	return fread(buffer, sizeof(Uint8), count, stream);
}

// 8-bit fread that dies if read fails
static inline void fread_u8_die(Uint8 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Uint8), count, stream);
}

// 8-bit fread that dies if read fails
static inline void fread_s8_die(Sint8 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Sint8), count, stream);
}

// 16-bit endian-swapping fread that dies if read fails
static inline void fread_u16_die(Uint16 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Uint16), count, stream);

#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	for (size_t i = 0; i < count; ++i)
		buffer[i] = SDL_Swap16(buffer[i]);
#endif
}

// 16-bit endian-swapping fread that dies if read fails
static inline void fread_s16_die(Sint16 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Sint16), count, stream);

#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	for (size_t i = 0; i < count; ++i)
		buffer[i] = SDL_Swap16(buffer[i]);
#endif
}

// 32-bit endian-swapping fread that dies if read fails
static inline void fread_u32_die(Uint32 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Uint32), count, stream);

#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	for (size_t i = 0; i < count; ++i)
		buffer[i] = SDL_Swap32(buffer[i]);
#endif
}

// 32-bit endian-swapping fread that dies if read fails
static inline void fread_s32_die(Sint32 *buffer, size_t count, FILE *stream)
{
	fread_die(buffer, sizeof(Sint32), count, stream);

#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	for (size_t i = 0; i < count; ++i)
		buffer[i] = SDL_Swap32(buffer[i]);
#endif
}

void fwrite_die(const void *buffer, size_t size, size_t count, FILE *stream);

// 8-bit fwrite that dies if write fails
static inline void fwrite_bool_die(const bool *buffer, FILE *stream)
{
	Uint8 temp = *buffer ? 1 : 0;
	fwrite_die(&temp, sizeof(Uint8), 1, stream);
}

// 8-bit fwrite
static inline size_t fwrite_u8(const Uint8 *buffer, size_t count, FILE *stream)
{
	return fwrite(buffer, sizeof(Uint8), count, stream);
}

// 8-bit fwrite that dies if write fails
static inline void fwrite_u8_die(const Uint8 *buffer, size_t count, FILE *stream)
{
	fwrite_die(buffer, sizeof(Uint8), count, stream);
}

// 8-bit fwrite that dies if write fails
static inline void fwrite_s8_die(const Sint8 *buffer, size_t count, FILE *stream)
{
	fwrite_die(buffer, sizeof(Sint8), count, stream);
}

// 16-bit endian-swapping fwrite that dies if write fails
static inline void fwrite_u16_die(const Uint16 *buffer, FILE *stream)
{
#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	Uint16 temp = SDL_Swap16(*buffer);
	buffer = &temp;
#endif

	fwrite_die(buffer, sizeof(Uint16), 1, stream);
}

// 16-bit endian-swapping fwrite that dies if write fails
static inline void fwrite_s16_die(const Sint16 *buffer, FILE *stream)
{
#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	Sint16 temp = SDL_Swap16(*buffer);
	buffer = &temp;
#endif

	fwrite_die(buffer, sizeof(Sint16), 1, stream);
}

// 32-bit endian-swapping fwrite that dies if write fails
static inline void fwrite_u32_die(const Uint32 *buffer, FILE *stream)
{
#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	Uint32 temp = SDL_Swap32(*buffer);
	buffer = &temp;
#endif

	fwrite_die(buffer, sizeof(Uint32), 1, stream);
}

// 32-bit endian-swapping fwrite that dies if write fails
static inline void fwrite_s32_die(const Sint32 *buffer, FILE *stream)
{
#if SDL_BYTEORDER == SDL_BIG_ENDIAN
	Sint32 temp = SDL_Swap32(*buffer);
	buffer = &temp;
#endif

	fwrite_die(buffer, sizeof(Sint32), 1, stream);
}

#endif // FILE_H
