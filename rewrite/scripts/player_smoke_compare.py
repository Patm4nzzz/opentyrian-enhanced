#!/usr/bin/env python3
"""Image tools of the player-smoke.sh check (Pillow + NumPy).

  crop    INPUT OUTPUT             1280x684 window -> exact 320x200 game image
  compare REF OBTAINED DIFF TOL    fraction of differing pixels; code 1 if > TOL
  changed A B THRESHOLD            code 0 if the two images differ by more than THRESHOLD
  blank   IMAGE                    code 1 if the image is nearly uniform (black screen)
  report  FOLDER                   writes report.md from results.tsv

The game is shown at an integer factor of 3, centered in the window: (160, 42)
to (1120, 642). Cutting by thirds is therefore exact, without smoothing.
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

BOX = (160, 42, 1120, 642)
BAD_PIXEL = 48  # max gap per channel beyond which a pixel counts as different


def load(path):
    return Image.open(path).convert("RGB")


def diff_fraction(a, b):
    """Share of pixels whose gap exceeds BAD_PIXEL, after a slight blur
    (a one-pixel shift of the scenery is not a regression)."""
    if a.size != b.size:
        return 1.0
    fa = np.asarray(a.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    fb = np.asarray(b.filter(ImageFilter.GaussianBlur(1)), dtype=np.int16)
    return float((np.abs(fa - fb).max(axis=2) > BAD_PIXEL).mean())


def cmd_crop(src, dst):
    im = load(src)
    if im.size != (1280, 684):
        sys.exit(f"capture of {im.size}, 1280x684 expected")
    im.crop(BOX).resize((320, 200), Image.NEAREST).save(dst, optimize=True)


def cmd_compare(ref, got, diff_out, tol, region=None):
    a, b = load(ref), load(got)
    if region:  # "x0,y0,x1,y1" in 320x200 game pixels: only that zone is compared (animated scenery around)
        zone = tuple(int(v) for v in region.split(","))
        a, b = a.crop(zone), b.crop(zone)
    frac = diff_fraction(a, b)
    if a.size == b.size:
        d = np.abs(np.asarray(a, dtype=np.int16) - np.asarray(b, dtype=np.int16)).max(axis=2)
        Image.fromarray(np.where(d > BAD_PIXEL, 255, d // 4).astype(np.uint8)).save(diff_out)
    print(f"{frac:.4f}")
    return 0 if frac <= float(tol) else 1


def cmd_changed(a, b, seuil, region=None):
    ia, ib = load(a), load(b)
    if region:
        # "x0,y0,x1,y1" in game pixels: only that zone counts, without blur
        # (a changing cash box digit only affects a few pixels).
        zone = tuple(int(v) for v in region.split(","))
        za = np.asarray(ia.crop(zone), dtype=np.int16)
        zb = np.asarray(ib.crop(zone), dtype=np.int16)
        return 0 if float((np.abs(za - zb).max(axis=2) > BAD_PIXEL).mean()) > float(seuil) else 1
    return 0 if diff_fraction(ia, ib) > float(seuil) else 1


def cmd_owned_row(path):
    """Row of the shop item list that carries the "owned"
    mark (the eye at x 298, y = 40 + 26 * row + 2, game_menu.c:JE_itemScreen).
    Prints the index (0 = first row) or -1 if no row stands out."""
    a = np.asarray(load(path), dtype=np.int16)
    zones = [a[40 + 26 * r : 40 + 26 * r + 14, 297:313] for r in range(5)]
    med = np.median(np.stack(zones), axis=0)
    score = [float(np.abs(z - med).mean()) for z in zones]
    best = int(np.argmax(score))
    others = sorted(score)[-2]
    print(best if score[best] > 12 and score[best] > 3 * others + 1 else -1)
    return 0


def cmd_blank(path):
    return 1 if np.asarray(load(path), dtype=np.float32).std() < 6 else 0


def cmd_report(folder):
    folder = Path(folder)
    rows = [
        line.split("\t") + [""] * 3
        for line in (folder / "results.tsv").read_text(encoding="utf-8").splitlines()
        if line
    ]
    bad = [r for r in rows if r[1] == "FAIL"]
    out = [
        "# player-smoke: report",
        "",
        f"Verdict: **{'FAILED' if bad else 'OK'}** ({len(rows) - len(bad)}/{len(rows)} checks passed).",
        "",
        "| Check | Result | Detail |",
        "|---|---|---|",
    ]
    out += [f"| {r[0]} | {r[1]} | {r[2]} |" for r in rows]
    shots = sorted((folder / "got").glob("*.png"))
    if shots:
        out += ["", "## Screenshots: reference | obtained | difference", ""]
        for p in shots:
            ref, diff = folder / "ref" / p.name, folder / "diff" / p.name
            failed = any(r[0] == "shot:" + p.stem and r[1] == "FAIL" for r in rows)
            out += [
                f"**{p.stem}**{'  (FAILED)' if failed else ''}",
                "",
                "| reference | obtained | difference |",
                "|---|---|---|",
                "| "
                + " | ".join(
                    [
                        f"![ref](ref/{p.name})" if ref.exists() else "(no reference)",
                        f"![obtained](got/{p.name})",
                        f"![difference](diff/{p.name})" if diff.exists() else "",
                    ]
                )
                + " |",
                "",
            ]
    (folder / "report.md").write_text("\n".join(out) + "\n", encoding="utf-8")


if __name__ == "__main__":
    cmd, args = sys.argv[1], sys.argv[2:]
    fn = {
        "crop": cmd_crop,
        "compare": cmd_compare,
        "changed": cmd_changed,
        "blank": cmd_blank,
        "report": cmd_report,
        "owned_row": cmd_owned_row,
    }[cmd]
    sys.exit(fn(*args) or 0)
