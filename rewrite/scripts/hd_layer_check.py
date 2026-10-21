#!/usr/bin/env python3
"""Missing-layer check in HD (AI) mode: same tick, Original versus HD.

Replays a golden master of the C version up to the requested tick, captures the play
area in Original mode then in HD mode (capture.gd, --hd-sprites=0|1), and
compares what is lit in the play area (264x184 logical pixels, excluding the
dashboard). A layer forgotten by the HD rendering (starfield, texts,
bars, truncated scenery) turns off part of the image that the Original shows.

Three measures. The first two, brightness threshold 24 (max of the three
components):
  - pixels: share of lit pixels;
  - cells : share of 8x8 logical-pixel cells that contain at least
    one lit pixel. Sensitive to scattered points: a missing starfield
    barely moves the share of pixels (under 1%), but turns off one
    cell in five.
  - blob  : largest group of neighboring 4x4 logical-pixel cells whose
    average color differs by more than 20 (out of 255) between the Original and the HD.
    The first two measures count what is lit; they do not see
    a missing darkening that leaves the image lit (level filter,
    lamp: 89% of HD pixels against 49%, but also a bright image), nor
    an object replaced by the scenery (player 2 ship missing). The blob
    sees them: upscaling changes the outlines, not the content of an area.
Gap reported if the share of cells differs by more than MAX_CELL_GAP, the share
of pixels by more than MAX_PIXEL_GAP (gaps relative to the Original), or if the
blob exceeds MAX_BLOB cells. Calibration of the first two (19 pairs,
episodes 1 to 5, Galaga, Squadron, Timed, two players, Endless): complete
renders between -15% and +12% in pixels (HD images softer, not emptier)
and between 0 and +7% in cells; an earlier development build, starfield missing: -18%
to -40% in cells on 6 pairs; layer 2 truncated (SQUADRON): -74% in pixels
and -62% in cells. Calibration of the blob (20 pairs from the chief's review):
the same build, filter and lamp missing 1328 to 1658 cells, player 2's Dragonwing
missing 29, enemies, shots and shadows out of the C's order 20 to 48; after
the fix, 12 at most (redrawn outlines of an explosion).
Non-regression cases for the defects found by the chief (REVIEW_CASES).

  hd_layer_check.py capture <godot_folder> <output> <case>...
      case = <golden.gm relative to golden/>:<tick>; with no case, those of the
      chief's review (REVIEW_CASES). Writes <name>-<tick>-hd0.png and -hd1.png
      (xvfb-run, isolated user profiles), then compares.
  hd_layer_check.py compare <output>
      compares the pairs already captured.
Exit code 1 if a pair is missing, if a capture fails or if a gap
exceeds the thresholds.
"""

import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

PLAY_W, PLAY_H = 264, 184
CELL = 8
THRESHOLD = 24
MAX_CELL_GAP = 0.15
MAX_PIXEL_GAP = 0.40
BLOB_CELL = 4
BLOB_DIFF = 20
MAX_BLOB = 16
# Chief's review: player 2's Dragonwing missing (2p-link), lamp missing
# (superarcade9), level filter missing (hard TYRIAN, Timed).
REVIEW_CASES = [
    "mode-2p-link.gm:900",
    "mode-superarcade9.gm:400",
    "e1-tyrian-hard-autopilot.gm:3000",
    "mode-timed1.gm:300",
]


def measure(path):
    """(share of pixels, share of cells) lit in the play area."""
    im = Image.open(path).convert("RGB")
    k = im.width / 320
    w, h = round(PLAY_W * k), round(PLAY_H * k)
    lit = [max(p) > THRESHOLD for p in im.crop((0, 0, w, h)).getdata()]
    cols, rows = PLAY_W // CELL, PLAY_H // CELL
    cells = [[False] * cols for _ in range(rows)]
    for i, on in enumerate(lit):
        if on:
            y, x = divmod(i, w)
            r, c = int(y / k) // CELL, int(x / k) // CELL
            if r < rows and c < cols:
                cells[r][c] = True
    return sum(lit) / len(lit), sum(map(sum, cells)) / (rows * cols)


def blob(path0, path1):
    """Largest group of BLOB_CELL² neighboring cells (4-connectivity) whose
    average color differs by more than BLOB_DIFF between the two images."""

    def cells(path):
        im = Image.open(path).convert("RGB").resize((320, 200), Image.BOX)
        cols, rows = PLAY_W // BLOB_CELL, PLAY_H // BLOB_CELL
        return (
            im.crop((0, 0, cols * BLOB_CELL, rows * BLOB_CELL)).resize((cols, rows), Image.BOX),
            cols,
            rows,
        )

    (a, cols, rows), (b, _, _) = cells(path0), cells(path1)
    pa, pb = a.load(), b.load()
    bad = {
        (x, y)
        for y in range(rows)
        for x in range(cols)
        if sum(abs(u - v) for u, v in zip(pa[x, y], pb[x, y], strict=False)) / 3 > BLOB_DIFF
    }
    best = 0
    while bad:
        todo, size = [bad.pop()], 0
        while todo:
            x, y = todo.pop()
            size += 1
            for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if n in bad:
                    bad.remove(n)
                    todo.append(n)
        best = max(best, size)
    return best


def compare(out):
    pairs = sorted(p for p in Path(out).glob("*-hd0.png"))
    if not pairs:
        print("no *-hd0.png / *-hd1.png pair", file=sys.stderr)
        return 1
    status = 0
    print(
        f"{'case':40} {'Orig pixels':>11} {'HD':>6} {'gap':>6} {'Orig cells':>10} {'HD':>6} {'gap':>6} {'blob':>5}  verdict"
    )
    for p0 in pairs:
        p1 = p0.with_name(p0.name.replace("-hd0.png", "-hd1.png"))
        name = p0.name[: -len("-hd0.png")]
        if not p1.exists():
            print(f"{name:40} HD capture missing")
            status = 1
            continue
        (px0, c0), (px1, c1) = measure(p0), measure(p1)
        dpx, dc = gap(px0, px1), gap(c0, c1)
        spot = blob(p0, p1)
        bad = abs(dc) > MAX_CELL_GAP or abs(dpx) > MAX_PIXEL_GAP
        status |= bad or spot > MAX_BLOB
        verdict = "GAP: missing layer?" if bad else "GAP: different area" if spot > MAX_BLOB else "ok"
        print(f"{name:40} {px0:11.3f} {px1:6.3f} {dpx:+6.0%} {c0:10.3f} {c1:6.3f} {dc:+6.0%} {spot:5d}  {verdict}")
    return status


def gap(orig, hd):
    """Relative gap of HD to the Original (a black Original image does not count)."""
    return (hd - orig) / orig if orig > 0 else 0.0


def capture(godot_dir, out, cases):
    godot = os.environ.get("GODOT", "godot")
    golden = Path(godot_dir).resolve().parent / "golden"
    Path(out).mkdir(parents=True, exist_ok=True)
    status = 0
    for case in cases:
        gm, tick = case.rsplit(":", 1)
        for hd in (0, 1):
            png = Path(out).resolve() / f"{Path(gm).stem}-{tick}-hd{hd}.png"
            with tempfile.TemporaryDirectory() as home:
                env = dict(
                    os.environ,
                    XDG_DATA_HOME=f"{home}/data",
                    XDG_CACHE_HOME=f"{home}/cache",
                    GODOT_SILENCE_ROOT_WARNING="1",
                )
                # C image of tick T = T + 1 simulation steps (capture_godot.sh).
                run = subprocess.run(
                    [
                        "xvfb-run",
                        "-a",
                        "-s",
                        "-screen 0 1280x800x24",
                        godot,
                        "--rendering-driver",
                        "opengl3",
                        "--audio-driver",
                        "Dummy",
                        "--path",
                        godot_dir,
                        "--",
                        f"--capture=level:{png}:{int(tick) + 1}",
                        f"--replay={golden / gm}",
                        f"--hd-sprites={hd}",
                    ],
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=900,
                )
            log = run.stdout + run.stderr
            png.with_suffix(".log").write_text(log)
            errors = [line for line in log.splitlines() if "ERROR" in line]
            if run.returncode != 0 or errors or not png.exists() or "IDENTICAL" not in log:
                print(f"capture failed: {png.name} (code {run.returncode}) {errors[:2]}")
                status = 1
    return status | compare(out)


def main(argv):
    if len(argv) >= 3 and argv[0] == "capture":
        return capture(argv[1], argv[2], argv[3:] or REVIEW_CASES)
    if len(argv) == 2 and argv[0] == "compare":
        return compare(argv[1])
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
