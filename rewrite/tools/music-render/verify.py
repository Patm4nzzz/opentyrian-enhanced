#!/usr/bin/env python3
"""Checks of the music rendering (ADR 0010).

Subcommands:
  loops  <music_render> <music.mus> <music.json> [backend]
      For each looping song: renders 10 s beyond loop_end and
      compares [loop_end, +10 s) with [loop_start, +10 s) (correlation, max gap).
  files  <music_dir>
      ffprobe of each OGG (duration, rate, channels) + decoded RMS/peak.
  compare <capture.raw> <music_render> <music.mus> <index> <backend> [music_volume]
      SDL disk capture (s16 stereo 44.1 kHz) of the C version against the tool's
      rendering: offset search, correlation, measured gain against the
      C gain (loudness.c: volumeFactorTable[musicVolume] * 2, Q20.12).
  fade <music_render> <music.mus> <index> [backend] [fade_at_ms]
      Measures the fade_song() envelope (lds_fade(1)): renders the song
      calling lds_fade(1) at fade_at_ms, then compares the energy per 50 ms
      window with that of the same passage rendered without a fade. Used to tune the fade
      of the Godot client (music.gd:FADE_SONG_SECONDS).
"""

import json
import math
import os
import subprocess
import sys
import tempfile

import numpy as np

RATE = 44100


def render(binary, mus, index, backend, extra_s=0, fade_at_ms=None):
    with tempfile.NamedTemporaryFile(suffix=".pcm", delete=False) as f:
        path = f.name
    args = [binary, mus, str(index), path, backend, str(RATE), str(extra_s)]
    if fade_at_ms is not None:
        args.append(str(fade_at_ms))
    try:
        out = subprocess.run(args, check=True, capture_output=True, text=True).stdout
        meta = json.loads(out.strip().splitlines()[-1])
        pcm = np.fromfile(path, dtype="<i2").astype(np.float64)
    finally:
        os.unlink(path)
    return meta, pcm


def corr(a, b):
    a = a - a.mean()
    b = b - b.mean()
    d = math.sqrt((a * a).sum() * (b * b).sum())
    return float((a * b).sum() / d) if d else 0.0


def cmd_loops(binary, mus, music_json, backend="nuked"):
    songs = json.load(open(music_json))["songs"]
    worst = 1.0
    for s in songs:
        if not s["loop"]:
            print("%2d %-26s no loop (end %.1f s)" % (s["index"], s["title"], s["duration"]))
            continue
        meta, pcm = render(binary, mus, s["index"], backend, extra_s=10)
        ls, le = meta["loop_start"], meta["loop_end"]
        n = min(10 * RATE, len(pcm) - le)
        a, b = pcm[ls : ls + n], pcm[le : le + n]
        c = corr(a, b)
        worst = min(worst, c)
        print(
            "%2d %-26s lap %.2f s  corr %.6f  max gap %5d  identical %s"
            % (
                s["index"],
                s["title"],
                (le - ls) / RATE,
                c,
                int(np.abs(a - b).max()),
                bool((a == b).all()),
            )
        )
    print("minimum correlation at the loop point: %.6f" % worst)


def cmd_files(music_dir):
    songs = json.load(open(os.path.join(music_dir, "music.json")))["songs"]
    bad = 0
    for s in songs:
        path = os.path.join(music_dir, s["file"])
        probe = json.loads(
            subprocess.run(
                [
                    "ffprobe",
                    "-v",
                    "error",
                    "-show_entries",
                    "format=duration:stream=sample_rate,channels,codec_name",
                    "-of",
                    "json",
                    path,
                ],
                check=True,
                capture_output=True,
                text=True,
            ).stdout
        )
        dur = float(probe["format"]["duration"])
        st = probe["streams"][0]
        raw = subprocess.run(
            ["ffmpeg", "-v", "error", "-i", path, "-f", "s16le", "-ac", "1", "-"],
            check=True,
            capture_output=True,
        ).stdout
        x = np.frombuffer(raw, dtype="<i2").astype(np.float64)
        rms = math.sqrt((x * x).mean()) if len(x) else 0.0
        db = 20 * math.log10(rms / 32768) if rms > 0 else -999
        ok = abs(dur - s["duration"]) < 0.05 and rms > 100 and st["sample_rate"] == str(RATE)
        bad += not ok
        print(
            "%2d %-26s %s %s Hz %d ch.  %7.2f s (expected %7.2f)  RMS %6.0f (%5.1f dBFS)  peak %5d  %s"
            % (
                s["index"],
                s["title"],
                st["codec_name"],
                st["sample_rate"],
                st["channels"],
                dur,
                s["duration"],
                rms,
                db,
                int(np.abs(x).max()),
                "OK" if ok else "FAILED",
            )
        )
    print("%d/%d files conform" % (len(songs) - bad, len(songs)))
    return 1 if bad else 0


def c_gain(music_volume):
    # loudness.c:init_audio, volumeRange = 30 dB, TO_FIXED Q20.12, then *2 for the OPL.
    if music_volume == 0:
        return 0.0
    f = np.float32(10.0) ** np.float32((255 - music_volume) * (-30.0 / (20.0 * 255)))
    return int(f * 4096) * 2 / 4096.0


def cmd_compare(capture, binary, mus, index, backend, music_volume=191):
    cap = np.fromfile(capture, dtype="<i2").astype(np.float64)
    left, right = cap[0::2], cap[1::2]
    print("capture: %.2f s, L == R: %s" % (len(left) / RATE, bool((left == right).all())))
    meta, ref = render(binary, mus, int(index), backend, extra_s=0)
    # Start of the song in the capture: first non-zero sample held.
    nz = np.flatnonzero(np.abs(left) > 4)
    if len(nz) == 0:
        print("silent capture")
        return 1
    # Offset search by FFT correlation over an 8 s window.
    win = 8 * RATE
    seg_ref = ref[:win]
    lo = max(0, nz[0] - RATE)
    hay = left[lo : lo + win + 4 * RATE]
    n = 1 << int(math.ceil(math.log2(len(hay) + len(seg_ref))))
    xc = np.fft.irfft(np.fft.rfft(hay, n) * np.conj(np.fft.rfft(seg_ref, n)), n)
    lag = int(np.argmax(xc[: len(hay)]))
    start = lo + lag
    m = min(len(left) - start, len(ref))
    a, b = left[start : start + m], ref[:m]
    c = corr(a, b)
    gain = float((a * b).sum() / (b * b).sum())
    expect = c_gain(int(music_volume))
    resid = a - np.clip(np.floor(b * expect * 4096 / 4096), -32768, 32767)
    print("start of song %d in the capture: %.4f s (frame %d)" % (int(index), start / RATE, start))
    print("overlap %.2f s, correlation %.6f" % (m / RATE, c))
    print("measured gain %.4f, C gain (musicVolume %d) %.4f" % (gain, int(music_volume), expect))
    print(
        "max gap after the C gain: %d (over %d samples), identical: %d"
        % (int(np.abs(resid).max()), m, int((np.abs(resid) <= 1).sum()))
    )
    return 0


def cmd_fade(binary, mus, index, backend="dosbox", fade_at_ms=10000):
    meta, faded = render(binary, mus, int(index), backend, fade_at_ms=int(fade_at_ms))
    _, plain = render(binary, mus, int(index), backend)
    f0 = meta["fade_started"]
    if f0 < 0:
        print("fade not triggered (song too short?)")
        return 1
    win = RATE // 20
    print("lds_fade(1) called at %.3f s (frame %d)" % (f0 / RATE, f0))
    print("  t (s)  RMS gain faded/not faded")
    silent_at = None
    t = 0
    while f0 + t + win <= min(len(faded), len(plain)):
        a = faded[f0 + t : f0 + t + win]
        b = plain[f0 + t : f0 + t + win]
        rb = math.sqrt((b * b).mean())
        g = math.sqrt((a * a).mean()) / rb if rb > 0 else 0.0
        if t % (RATE // 4) < win:
            print("  %5.2f  %.4f" % (t / RATE, g))
        if silent_at is None and g < 0.01:
            silent_at = t / RATE
        t += win
    print(
        "gain < 1 %% (-40 dB) reached at %s s after fade_song()"
        % ("%.2f" % silent_at if silent_at is not None else "—")
    )
    return 0


if __name__ == "__main__":
    cmd, args = sys.argv[1], sys.argv[2:]
    sys.exit({"loops": cmd_loops, "files": cmd_files, "compare": cmd_compare, "fade": cmd_fade}[cmd](*args) or 0)
