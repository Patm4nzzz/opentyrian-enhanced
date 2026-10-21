/*
 * Game symbols referenced by lds_play.c / opl*.c, provided here so that
 * the tool compiles those files without the rest of the C version (file.c pulls in
 * varz.h and the whole game, log.c the log, loudness.c SDL audio).
 * Behavior identical to src/file.c and src/log.c for what is used.
 */
#include "opentyr.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

/* loudness.c */
int audioSampleRate = 0;

/* file.c:fread_die */
void fread_die(void *buffer, size_t size, size_t count, FILE *stream)
{
	if (fread(buffer, size, count, stream) != count)
	{
		fprintf(stderr, "file: unexpected read error\n");
		exit(EXIT_FAILURE);
	}
}

/* file.c:ftell_eof */
long ftell_eof(FILE *f)
{
	long pos = ftell(f);
	fseek(f, 0, SEEK_END);
	long size = ftell(f);
	fseek(f, pos, SEEK_SET);
	return size;
}

/* log.c */
static void vlog(const char *level, const char *fmt, va_list ap)
{
	fprintf(stderr, "%s: ", level);
	vfprintf(stderr, fmt, ap);
	fputc('\n', stderr);
}

void log_info(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vlog("info", fmt, ap); va_end(ap); }
void log_warn(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vlog("warn", fmt, ap); va_end(ap); }
void log_error(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vlog("error", fmt, ap); va_end(ap); }
