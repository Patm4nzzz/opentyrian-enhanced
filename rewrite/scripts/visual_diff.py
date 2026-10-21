#!/usr/bin/env python3
"""Visual parity C / Godot.

Subcommands:
  otfr2png IN.otfr OUT.png [--scale N]
      Image written by the instrumented C build (GOLDEN_FRAMES, golden_master.c:
      8-bit framebuffer + current palette + RGBA layer) -> 320x200 RGB PNG.
      The RGBA layer is composited like SDL_BLENDMODE_BLEND (video_rgba.c).
  diff A.png B.png [--out DIFF.png] [--scale N]
      Compares two RGB images of the same size (xN images are scaled back to
      320x200 if --scale N). Prints "different=<n> total=<t>" and writes a
      difference image: equal pixels in dark gray, differing ones in red.
  sidebyside A.png B.png OUT.png [--scale N]
      Juxtaposes C | Godot | difference, enlarged xN (report).
  frames DIR
      Lists the images of a GOLDEN_FRAMES folder (number, tick, fingerprint).
  seqdiff C_DIR GODOT_DIR [--png OUT_DIR] [--rank N] [--skip-c N] [--skip-g N] [--trim-c]
      Compares two OTFR sequences image by image (same rank): for each rank,
      number of differing RGB pixels. With --png, writes the last image of
      each side and the side-by-side pair of the first gap (or the last one).
      Return code 0 if both sequences are identical (same length).
      --skip-c / --skip-g: leading ranks ignored; --trim-c: C sequence cut
      to the length of the Godot sequence. C_DIR may be a fingerprint file
      written by seqhash (versioned C reference): a rank then differs if
      the fingerprint of the Godot image is not the C one (--png has no effect).
  seqhash DIR
      SHA-1 fingerprint of each composed image of an OTFR folder, one per line.

Dependency: Pillow only (no numpy).
"""

import hashlib
import struct
import sys
from pathlib import Path

from PIL import Image


def read_otfr(path):
    data = Path(path).read_bytes()
    if data[:4] != b"OTFR":
        raise SystemExit(f"{path}: not an OTFR file")
    tick, w, h = struct.unpack_from("<IHH", data, 4)
    off = 12
    idx = data[off : off + w * h]
    off += w * h
    pal = data[off : off + 768]
    off += 768
    rgba = data[off : off + w * h * 4]
    return tick, w, h, idx, pal, rgba


def otfr_image(path):
    tick, w, h, idx, pal, rgba = read_otfr(path)
    out = bytearray(w * h * 3)
    for i, c in enumerate(idx):
        out[3 * i : 3 * i + 3] = pal[3 * c : 3 * c + 3]
    # ARGB8888 layer (little-endian memory: B G R A) composited as
    # SDL_BLENDMODE_BLEND: dst = src*a + dst*(1-a).
    if any(rgba[3::4]):
        for i in range(w * h):
            a = rgba[4 * i + 3]
            if a == 0:
                continue
            b, g, r = rgba[4 * i], rgba[4 * i + 1], rgba[4 * i + 2]
            for k, s in enumerate((r, g, b)):
                d = out[3 * i + k]
                out[3 * i + k] = (s * a + d * (255 - a)) // 255
    return tick, Image.frombytes("RGB", (w, h), bytes(out))


def image_hash(img):
    """Fingerprint of a composed RGB image (versioned C reference, seqhash)."""
    return hashlib.sha1(img.tobytes()).hexdigest()


def load_rgb(path, scale):
    img = Image.open(path).convert("RGB")
    if scale > 1:
        w, h = img.size
        img = img.resize((w // scale, h // scale), Image.NEAREST)
    return img


def diff_images(a, b):
    if a.size != b.size:
        raise SystemExit(f"different sizes: {a.size} / {b.size}")
    pa, pb = a.tobytes(), b.tobytes()
    w, h = a.size
    out = bytearray(w * h * 3)
    n = 0
    for i in range(w * h):
        x = pa[3 * i : 3 * i + 3]
        if x != pb[3 * i : 3 * i + 3]:
            n += 1
            out[3 * i : 3 * i + 3] = b"\xff\x00\x00"
        else:
            g = (x[0] + x[1] + x[2]) // 9
            out[3 * i : 3 * i + 3] = bytes((g, g, g))
    return n, w * h, Image.frombytes("RGB", (w, h), bytes(out))


def arg(args, name, default=None):
    if name in args:
        i = args.index(name)
        v = args[i + 1]
        del args[i : i + 2]
        return v
    return default


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    cmd, args = argv[0], argv[1:]
    scale = int(arg(args, "--scale", "1"))
    if cmd == "otfr2png":
        tick, img = otfr_image(args[0])
        if scale > 1:
            img = img.resize((img.width * scale, img.height * scale), Image.NEAREST)
        img.save(args[1])
        print(f"tick={tick if tick != 0xFFFFFFFF else -1} -> {args[1]}")
    elif cmd == "diff":
        out = arg(args, "--out")
        a, b = load_rgb(args[0], scale), load_rgb(args[1], scale)
        n, total, d = diff_images(a, b)
        if out:
            d.save(out)
        print(f"different={n} total={total}")
        return 0 if n == 0 else 1
    elif cmd == "sidebyside":
        a, b = load_rgb(args[0], 1), load_rgb(args[1], 1)
        if a.size != b.size:
            b = b.resize(a.size, Image.NEAREST)
        n, total, d = diff_images(a, b)
        w, h = a.size
        sheet = Image.new("RGB", (w * 3 + 8, h), (40, 40, 40))
        sheet.paste(a, (0, 0))
        sheet.paste(b, (w + 4, 0))
        sheet.paste(d, (2 * w + 8, 0))
        if scale > 1:
            sheet = sheet.resize((sheet.width * scale, sheet.height * scale), Image.NEAREST)
        sheet.save(args[2])
        print(f"different={n} total={total}")
    elif cmd == "seqdiff":
        png = arg(args, "--png")
        rank = arg(args, "--rank")
        # --skip-c / --skip-g: ranks ignored at the head of each sequence (shop:
        # level played on the C side, title animation on the Godot side).
        skip_c = int(arg(args, "--skip-c", "0"))
        skip_g = int(arg(args, "--skip-g", "0"))
        # Versioned C reference: fingerprint file (seqhash subcommand)
        # instead of the image folder; a rank differs if the fingerprint differs.
        c_hashes = Path(args[0]).is_file()
        if c_hashes:
            ca = Path(args[0]).read_text().split()[skip_c:]
            png = None
        else:
            ca = sorted(Path(args[0]).glob("*.otfr"))[skip_c:]
        ga = sorted(Path(args[1]).glob("*.otfr"))[skip_g:]
        if "--trim-c" in args:  # longer C sequence (shop: next level)
            args.remove("--trim-c")
            ca = ca[: len(ga)]
        if not ca or not ga:
            print("no OTFR image to compare", file=sys.stderr)
            return 1
        first = None
        ndiff = 0
        for i in range(max(len(ca), len(ga))):
            if i >= len(ca) or i >= len(ga):
                print(f"{i:06d} missing on the {'C' if i >= len(ca) else 'Godot'} side")
                first = i if first is None else first
                ndiff += 1
                continue
            _, b = otfr_image(ga[i])
            if c_hashes:
                if image_hash(b) != ca[i]:
                    ndiff += 1
                    print(f"{i:06d} different fingerprint")
                    first = i if first is None else first
                continue
            _, a = otfr_image(ca[i])
            n, total, _ = diff_images(a, b)
            if n:
                ndiff += 1
                print(f"{i:06d} different={n}/{total}")
                first = i if first is None else first
        print(f"images C={len(ca)} Godot={len(ga)} different ranks={ndiff} first={first}")
        if png and ca and ga:
            Path(png).mkdir(parents=True, exist_ok=True)
            k = first if first is not None and first < min(len(ca), len(ga)) else min(len(ca), len(ga)) - 1
            if first is None and rank is not None:
                k = int(rank)  # pair of a chosen rank when everything is identical
            _, a = otfr_image(ca[k])
            _, b = otfr_image(ga[k])
            a.save(Path(png) / "c.png")
            b.save(Path(png) / "godot.png")
            n, total, d = diff_images(a, b)
            sheet = Image.new("RGB", (a.width * 3 + 8, a.height), (40, 40, 40))
            sheet.paste(a, (0, 0))
            sheet.paste(b, (a.width + 4, 0))
            sheet.paste(d, (2 * a.width + 8, 0))
            sheet.resize((sheet.width * 2, sheet.height * 2), Image.NEAREST).save(Path(png) / "pair.png")
            print(f"rank {k}: different={n}/{total} -> {png}")
        return 0 if ndiff == 0 else 1
    elif cmd == "seqhash":
        # One SHA-1 fingerprint per composed image (RGB 320x200), in order.
        for f in sorted(Path(args[0]).glob("*.otfr")):
            print(image_hash(otfr_image(f)[1]))
    elif cmd == "frames":
        d = Path(args[0])
        for f in sorted(d.glob("*.otfr")):
            tick, w, h, idx, pal, rgba = read_otfr(f)
            hsh = hashlib.sha1(idx + pal + rgba).hexdigest()[:12]
            print(f.stem, -1 if tick == 0xFFFFFFFF else tick, hsh)
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
