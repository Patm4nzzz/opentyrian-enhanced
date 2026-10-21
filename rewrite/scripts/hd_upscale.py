#!/usr/bin/env python3
"""HD mode (AI) images, upscaled x4 by the 4x-PixelPerfectV4 model (ADR 0100).

Scenery: whole level maps.
Upscaling tile by tile leaves a grid: a tile shared by hundreds of
places cannot blend with all its neighbors. Here each layer of each
level is rebuilt as a single image (24x28 tiles),
upscaled x4 in one block by the 4x-PixelPerfectV4 model, then cut into
horizontal bands that the game displays during scrolling (scripts/hd_map_layer.gd).
Each WebP band also contains BAND_MARGIN pixels of its neighbors, which the
game does not display: encoded alone, a band blends poorly with the next one
(seam ratio at band edges 1.37 against 1.04 with the margin).

The model smooths fine grain (grass): where the upscaled image has lost
most of the original's grain, that grain is restored from the original
(see restore_grain).

  hd_upscale.py sprites <imported> <model.pth> <output> [name...]
      writes <output>/sprites/<name>.png: each frame upscaled alone with a
      mirrored border (atlas_pad.py), atlas JSON rectangles x4.
      The short-effect sheets (SHARP_SHEETS: explosions, smoke,
      debris, shots) do not go through the model, which turns them into bubbles:
      they are upscaled by xBRZ, crisp pixel art (see sharp_sprites).
  hd_upscale.py frames <imported> <HD folder>
      frame-by-frame Original versus HD check of all the sheets
      (exact x4, shift, mixed-up frames, holes); code 1 at the first defect.
  hd_upscale.py sharp <imported> <output> [name...]
      rewrites only the SHARP_SHEETS sheets (or the named ones) with
      xBRZ x4, frame by frame; no GPU. Requires the xbrz.py 1.0.2 module.
  hd_upscale.py interface <imported> <model.pth> <output>
      writes <output>/pics/pic_NN.png (full-screen images of tyrian.pic:
      menu backgrounds, title, shop, dashboard frame) and
      <output>/sprites/main_faces/face_NN.png (portraits, each with its
      palette) and <output>/sprites/palNN/*.png (atlases in the palette of an
      interface screen, frame by frame), upscaled x4 by the model (interface
      HD mode).
  hd_upscale.py fidelity <imported> <hd_output> <name>...
      mean gap (0-255) between the original and the upscaled sheet scaled back to
      its original size (4x4 average): lower = more faithful.
  hd_upscale.py render <imported> <model.pth> <output> [--quality Q] [level...]
      writes <output>/maps/<key>/{layer.json,NNN.webp} for each distinct
      layer. Requires CUDA (torch + spandrel); deterministic output.
  hd_upscale.py key <imported>
      lists "key level layer" (same computation as HdMapLayer.key in GDScript).
  hd_upscale.py digest <imported>
      SHA-256 fingerprint of the tile grids (archive is stale if a level
      changes tiles: scripts/hd_assets.sh). Pure Python, no NumPy.
  hd_upscale.py measure <imported> <output> [level...]
      seam score per layer, on the written bands (no GPU):
      jump at the tile edge / jump at the other x4 pixel edges.
  hd_upscale.py verify <imported> <model.pth> <output> <level...>
      before/after table: same score on the model's uncut image
      (reference, no band seam possible) and on the bands.
"""

import hashlib
import json
import os
import struct
import sys

TW, TH, K = 24, 28, 4
CHUNK_ROWS = 4  # tile rows per displayed band (448 px high x4)
MODEL_SHA256 = "52bcda86effc93e07d868036f53dd2f4a38016cdd636b66ebbf6c4525223eb5f"
INFER_TILE = 256  # useful side of an inference chunk (input pixels)
INFER_PAD = 32  # context around each chunk, discarded on output
WEBP_QUALITY = 90  # WebP quality of the bands (--quality to lower it)
BAND_MARGIN = 16  # x4 pixels of the neighboring bands encoded around each band
# Sheets upscaled by xBRZ and not by the model: short effects, which the
# model redraws as round bubbles or glowing balls (newsh6:
# explosions, smoke, debris; main_07 / main_11: shots). Mean gap to
# the original (fidelity): newsh6 7.96 AI against 3.69 xBRZ; shots 8.37 / 9.28
# against 4.79 / 5.27.
# Fonts (main_00..02, and their Christmas version): the model distorts the
# small letters and shifts glyphs (chief's review); xBRZ keeps them
# identical and crisp.
FONT_SHEETS = tuple(f"main{x}_{n}" for x in ("", "_christmas") for n in ("00_font", "01_small_font", "02_tiny_font"))
SHARP_SHEETS = (
    "newsh6",
    "main_07_player_shots",
    "main_11_player_shots2",
    "main_christmas_07_player_shots",
    "main_christmas_11_player_shots2",
) + FONT_SHEETS
# Atlases in the game palette that the C never draws that way (planets:
# palettes 8 and 17, portraits: facepal; estpa is not loaded by the C):
# not upscaled, otherwise the model starts from an image with the wrong colors.
SKIP_SHEETS = (
    "main_03_planets",
    "main_04_faces",
    "main_christmas_03_planets",
    "main_christmas_04_faces",
    "estpa",
)
# `frames` check: frames with at least `min_pixels` opaque pixels; defect
# if more than `hole` of the opaque pixels are transparent in HD, or if a
# shift / another frame brings the gap below `better` x the frame's gap
# (only when the gap exceeds `min_err`: below that, noise dominates).
FRAME_LIMITS = {"min_pixels": 6, "hole": 0.15, "better": 0.6, "min_err": 10.0}
SHARP_PAD = 2  # transparent border around each frame for xBRZ


# --- Grids (pure Python: digest and key run without NumPy) --------------


def layers(imported):
    """[(key, tileset, level, layer, width, height, normalized tiles)]
    without duplicates: two levels with an identical layer share the key."""
    seen, out, metas = set(), [], {}
    for name in sorted(os.listdir(os.path.join(imported, "levels"))):
        if not name.endswith(".json"):
            continue
        d = json.load(open(os.path.join(imported, "levels", name), encoding="utf-8"))
        ts = d["tileset"]
        if ts not in metas:
            meta = json.load(open(os.path.join(imported, "tiles", ts + ".json"), encoding="utf-8"))
            metas[ts] = set(int(n) for n in meta["blank_tiles"])
        for li, layer in enumerate(d["layers"]):
            tiles = [0 if (v < 0 or v in metas[ts]) else v for v in layer["tiles"]]
            if not any(tiles):
                continue
            k = key(ts, layer["width"], layer["height"], tiles)
            if k in seen:
                continue
            seen.add(k)
            out.append((k, ts, name[:-5], li, layer["width"], layer["height"], tiles))
    return out


def key(tileset, width, height, tiles):
    """Folder name of a layer: SHA-256 (16 hex) of "tileset l h\\n" then
    of the tile numbers as little-endian int32 (0 = empty)."""
    h = hashlib.sha256(f"{tileset} {width} {height}\n".encode())
    h.update(struct.pack(f"<{len(tiles)}i", *tiles))
    return h.hexdigest()[:16]


def digest(imported):
    h = hashlib.sha256()
    for k, ts, lvl, li, _w, _hh, _ in layers(imported):
        h.update(f"{k} {ts} {lvl} {li}\n".encode())
    return h.hexdigest()


# --- Rendering -------------------------------------------------------------------


def compose(np, tiles, w, h, atlas, cols):
    img = np.zeros((h * TH, w * TW, 4), np.uint8)
    for i, n in enumerate(tiles):
        if n:
            r, c = divmod(i, w)
            ax, ay = (n - 1) % cols * TW, (n - 1) // cols * TH
            img[r * TH : (r + 1) * TH, c * TW : (c + 1) * TW] = atlas[ay : ay + TH, ax : ax + TW]
    return img


def upscale(np, torch, model, rgb):
    """x4 in overlapping chunks of which only the center is kept: the inference
    tiling introduces no seam."""
    h, w, _ = rgb.shape
    p = INFER_PAD
    src = np.pad(rgb, ((p, p), (p, p), (0, 0)), mode="reflect").astype(np.float32) / 255.0
    out = np.zeros((h * K, w * K, 3), np.float32)
    with torch.no_grad():
        for y in range(0, h, INFER_TILE):
            for x in range(0, w, INFER_TILE):
                th, tw = min(INFER_TILE, h - y), min(INFER_TILE, w - x)
                t = torch.from_numpy(src[y : y + th + 2 * p, x : x + tw + 2 * p]).permute(2, 0, 1)[None].cuda()
                r = model(t)[0].permute(1, 2, 0).clamp(0, 1).cpu().numpy()
                out[y * K : (y + th) * K, x * K : (x + tw) * K] = r[p * K : (p + th) * K, p * K : (p + tw) * K]
    return out * 255.0


def blur(torch, t, sigma):
    """Separable Gaussian blur of a (C, H, W) tensor, replicated borders."""
    import torch.nn.functional as F

    r = int(3 * sigma + 0.5)
    x = torch.arange(-r, r + 1, dtype=torch.float32, device=t.device)
    k = torch.exp(-x * x / (2 * sigma * sigma))
    k = k / k.sum()
    t = F.pad(t[:, None], (r, r, r, r), mode="replicate")
    t = F.conv2d(t, k.view(1, 1, 1, -1))
    return F.conv2d(t, k.view(1, 1, -1, 1))[:, 0]


def restore_grain(np, torch, big, orig):
    """Restores the fine grain that the model smoothed (grass, sand).

    `orig`: original x4 nearest-neighbor then softened (σ 0.9: 4 px steps
    rounded). The energy of the fine details (band σ < 3) of the two
    images is compared, averaged over σ 8: where the upscaled image keeps less than
    half of it, its fine details are replaced by those of the original; elsewhere
    (stone, metal, outlines) it stays intact."""
    b = torch.from_numpy(big).permute(2, 0, 1).cuda()
    o = blur(torch, torch.from_numpy(orig).permute(2, 0, 1).cuda(), 0.9)
    bh, oh = b - blur(torch, b, 3.0), o - blur(torch, o, 3.0)
    eb = blur(torch, (bh * bh).mean(0, keepdim=True), 8.0).sqrt()
    eo = blur(torch, (oh * oh).mean(0, keepdim=True), 8.0).sqrt()
    lost = ((eo - eb) / (eo + 1e-3) * 2).clamp(0, 1)
    return (b + lost * (oh - bh)).clamp(0, 255).permute(1, 2, 0).cpu().numpy()


def write_manifest(out_dir, **kv):
    """Completes <output>/manifest.json (provenance of the HD archive)."""
    import datetime

    path = os.path.join(out_dir, "manifest.json")
    m = {}
    if os.path.exists(path):
        m = json.load(open(path, encoding="utf-8"))
    m.update(kv)
    m["date"] = datetime.datetime.now().isoformat(timespec="seconds")
    json.dump(m, open(path, "w", encoding="utf-8"), ensure_ascii=False, indent=2, sort_keys=True)


def load_model(model_path):
    import spandrel
    import torch

    got = hashlib.sha256(open(model_path, "rb").read()).hexdigest()
    if got != MODEL_SHA256:
        sys.exit(f"unexpected model (SHA-256 {got}, expected {MODEL_SHA256})")
    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False
    torch.backends.cuda.matmul.allow_tf32 = False
    torch.backends.cudnn.allow_tf32 = False
    return spandrel.ModelLoader().load_from_file(model_path).cuda().eval()


def upscale_rgba(np, torch, model, img):
    """RGB through the model; alpha outline upscaled by the same model then thresholded
    at mid-height (all or nothing like the original, no staircase)."""
    big = upscale(np, torch, model, img[..., :3])
    alpha = img[..., 3]
    if alpha.min() == 255:
        return big, None
    a = upscale(np, torch, model, np.repeat(alpha[..., None], 3, axis=2)).mean(axis=2)
    return big, np.where(a >= 127.5, 255, 0).astype(np.uint8)


def upscale_atlas(np, torch, Image, atlas_pad, model, png, meta, out_png, work):
    """Atlas upscaled frame by frame (mirrored border from atlas_pad.py)."""
    os.makedirs(work, exist_ok=True)
    os.makedirs(os.path.dirname(out_png), exist_ok=True)
    atlas_pad.pad(png, meta, os.path.join(work, "cases.png"), os.path.join(work, "cases.json"))
    cases = np.array(Image.open(os.path.join(work, "cases.png")).convert("RGBA"))
    big, alpha = upscale_rgba(np, torch, model, cases)
    out = big.round().astype(np.uint8)
    out = np.dstack([out, alpha if alpha is not None else np.full(out.shape[:2], 255, np.uint8)])
    Image.fromarray(out).save(os.path.join(work, "cases-x4.png"))
    atlas_pad.unpad(os.path.join(work, "cases-x4.png"), os.path.join(work, "cases.json"), out_png)


def sprites(imported, model_path, out_dir, names=()):
    import numpy as np
    import torch
    from PIL import Image

    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import atlas_pad

    model = load_model(model_path)
    src = os.path.join(imported, "sprites")
    dst = os.path.join(out_dir, "sprites")
    os.makedirs(dst, exist_ok=True)
    for f in sorted(os.listdir(src)):
        if not f.endswith(".png") or (names and f[:-4] not in names) or f[:-4] in SHARP_SHEETS + SKIP_SHEETS:
            continue
        meta = os.path.join(src, f[:-4] + ".json")
        if not os.path.exists(meta):
            continue
        upscale_atlas(
            np,
            torch,
            Image,
            atlas_pad,
            model,
            os.path.join(src, f),
            meta,
            os.path.join(dst, f),
            os.path.join(out_dir, "_work"),
        )
        print(f"hd_upscale : sprites/{f}", flush=True)
    import shutil

    shutil.rmtree(os.path.join(out_dir, "_work"), ignore_errors=True)
    sharp_sprites(imported, out_dir, [n for n in SHARP_SHEETS if not names or n in names])
    write_manifest(
        out_dir,
        sprites=sorted(os.listdir(dst)),
        model="4x-PixelPerfectV4",
        model_sha256=MODEL_SHA256,
        scale=K,
        frame_padding="6 px mirror padding per frame (atlas_pad.py)",
        tool="hd_upscale.py sprites (spandrel + PyTorch, fp32)",
    )


def interface(imported, model_path, out_dir):
    """Interface images: each full-screen image and each portrait is
    upscaled alone, in one block (mirrored borders by `upscale`)."""
    import numpy as np
    import torch
    from PIL import Image

    model = load_model(model_path)
    jobs = []
    for sub in (
        "pics",
        os.path.join("sprites", "main_faces"),
        os.path.join("sprites", "main_christmas_faces"),
    ):
        src = os.path.join(imported, sub)
        if os.path.isdir(src):
            jobs += [(sub, f) for f in sorted(os.listdir(src)) if f.endswith(".png")]
    for sub, f in jobs:
        img = np.array(Image.open(os.path.join(imported, sub, f)).convert("RGBA"))
        big, alpha = upscale_rgba(np, torch, model, img)
        out = big.round().astype(np.uint8)
        out = np.dstack([out, alpha if alpha is not None else np.full(out.shape[:2], 255, np.uint8)])
        os.makedirs(os.path.join(out_dir, sub), exist_ok=True)
        Image.fromarray(out).save(os.path.join(out_dir, sub, f))
        print(f"hd_upscale : {sub}/{f}", flush=True)
    # Atlases exported in the palette of an interface screen (sprites/palNN/).
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import atlas_pad

    sprites_dir = os.path.join(imported, "sprites")
    for sub in sorted(d for d in os.listdir(sprites_dir) if d.startswith("pal")):
        for f in sorted(os.listdir(os.path.join(sprites_dir, sub))):
            if not f.endswith(".png"):
                continue
            rel = os.path.join("sprites", sub, f)
            upscale_atlas(
                np,
                torch,
                Image,
                atlas_pad,
                model,
                os.path.join(imported, rel),
                os.path.join(imported, rel[:-4] + ".json"),
                os.path.join(out_dir, rel),
                os.path.join(out_dir, "_work"),
            )
            jobs.append((sub, f))
            print(f"hd_upscale : {rel}", flush=True)
    import shutil

    shutil.rmtree(os.path.join(out_dir, "_work"), ignore_errors=True)
    write_manifest(
        out_dir,
        interface_images=len(jobs),
        interface_tool="hd_upscale.py interface (4x-PixelPerfectV4)",
    )


def sharp_sprites(imported, out_dir, names=SHARP_SHEETS):
    """Short-effect sheets: each frame upscaled x4 by xBRZ, alone, with
    SHARP_PAD transparent pixels around it (no atlas neighbor spills over);
    atlas JSON rectangles x4 like the other sheets."""
    import xbrz
    from PIL import Image

    src = os.path.join(imported, "sprites")
    dst = os.path.join(out_dir, "sprites")
    os.makedirs(dst, exist_ok=True)
    names = [n for n in names if os.path.exists(os.path.join(src, n + ".png"))]
    for name in names:
        atlas = Image.open(os.path.join(src, name + ".png")).convert("RGBA")
        meta = json.load(open(os.path.join(src, name + ".json"), encoding="utf-8"))
        out = Image.new("RGBA", (atlas.width * K, atlas.height * K))
        p = SHARP_PAD
        for fr in meta["frames"]:
            x, y, w, h = fr["x"], fr["y"], fr["w"], fr["h"]
            if w <= 0 or h <= 0:
                continue
            case = Image.new("RGBA", (w + 2 * p, h + 2 * p))
            case.paste(atlas.crop((x, y, x + w, y + h)), (p, p))
            big = xbrz.scale_pillow(case, K)
            out.paste(big.crop((p * K, p * K, (p + w) * K, (p + h) * K)), (x * K, y * K))
        out.save(os.path.join(dst, name + ".png"))
        print(f"hd_upscale : sprites/{name}.png (xBRZ)", flush=True)
    write_manifest(out_dir, sharp_sheets=list(names), sharp_tool="xBRZ ×4 (xbrz.py 1.0.2), frame by frame")


def fidelity(imported, hd_dir, names):
    import numpy as np
    from PIL import Image

    for name in names:
        o = np.array(Image.open(os.path.join(imported, "sprites", name + ".png")).convert("RGBA")).astype(np.float32)
        b = np.array(Image.open(os.path.join(hd_dir, "sprites", name + ".png")).convert("RGBA")).astype(np.float32)
        b = b.reshape(o.shape[0], K, o.shape[1], K, 4).mean(axis=(1, 3))
        m = o[..., 3] > 0
        print(f"{name} {np.abs(o[..., :3] - b[..., :3])[m].mean():.2f}")


def frames(imported, hd_dir):
    """Original versus HD check, frame by frame, of all the upscaled
    sheets: image exactly x4; each frame scaled back to x1 (4x4 average)
    compared to the opaque pixels of the original; a shift of 1 to 3 px or another
    frame of the same size that would match clearly better (shifted
    cut, mixed-up frames); holes (HD transparent where the original is
    opaque). Exit code 1 at the first defect. Mean gap per sheet
    displayed (thresholds in FRAME_LIMITS)."""
    import numpy as np
    from PIL import Image

    src = os.path.join(imported, "sprites")
    failures, frames_seen, sheets_seen = [], 0, 0
    for jf in sorted(os.listdir(src)):
        if not jf.endswith(".json"):
            continue
        name = jf[:-5]
        hd_path = os.path.join(hd_dir, "sprites", name + ".png")
        if not os.path.exists(hd_path):
            continue
        meta = json.load(open(os.path.join(src, jf), encoding="utf-8"))
        o = np.asarray(Image.open(os.path.join(src, name + ".png")).convert("RGBA")).astype(np.float32)
        h = Image.open(hd_path).convert("RGBA")
        sheets_seen += 1
        if h.size != (o.shape[1] * K, o.shape[0] * K):
            failures.append(f"{name}: {h.size[0]}×{h.size[1]} instead of {o.shape[1] * K}×{o.shape[0] * K}")
            continue
        hd = np.asarray(h).astype(np.float32).reshape(o.shape[0], K, o.shape[1], K, 4).mean(axis=(1, 3))
        crops = {}
        for f in meta["frames"]:
            x, y, w, fh = f["x"], f["y"], f["w"], f["h"]
            if w > 0 and fh > 0:
                crops[f["index"]] = (x, y, w, fh)
        errs = []
        for idx, (x, y, w, fh) in crops.items():
            oc, hc = o[y : y + fh, x : x + w], hd[y : y + fh, x : x + w]
            m = oc[..., 3] > 128
            if m.sum() < FRAME_LIMITS["min_pixels"]:
                continue
            frames_seen += 1
            err = float(np.abs(oc[..., :3] - hc[..., :3])[m].mean())
            errs.append(err)
            hole = float(((hc[..., 3] < 64) & m).sum() / m.sum())
            if hole > FRAME_LIMITS["hole"]:
                failures.append(f"{name}#{idx} : {hole:.0%} de trous")
            if err <= FRAME_LIMITS["min_err"]:
                continue
            for dx in range(-3, 4):
                for dy in range(-3, 4):
                    x2, y2 = x + dx, y + dy
                    if (dx or dy) and x2 >= 0 and y2 >= 0 and x2 + w <= o.shape[1] and y2 + fh <= o.shape[0]:
                        e2 = float(np.abs(oc[..., :3] - hd[y2 : y2 + fh, x2 : x2 + w, :3])[m].mean())
                        if e2 < err * FRAME_LIMITS["better"]:
                            failures.append(f"{name}#{idx}: shifted by ({dx}, {dy}) px ({err:.1f} -> {e2:.1f})")
                            break
                else:
                    continue
                break
            for j, (x3, y3, w3, h3) in crops.items():
                if j != idx and (w3, h3) == (w, fh):
                    e3 = float(np.abs(oc[..., :3] - hd[y3 : y3 + h3, x3 : x3 + w3, :3])[m].mean())
                    if e3 < err * FRAME_LIMITS["better"]:
                        failures.append(f"{name}#{idx}: looks more like frame {j} ({err:.1f} -> {e3:.1f})")
                        break
        if errs:
            print(f"{name:34s} {len(errs):5d} frames  mean error {np.mean(errs):5.1f}  max {np.max(errs):5.1f}")
    print(f"{sheets_seen} sheets, {frames_seen} frames, {len(failures)} defect(s)")
    for f in failures[:50]:
        print("  " + f)
    sys.exit(1 if failures else 0)


def render(imported, model_path, out_dir, levels=(), quality=WEBP_QUALITY):
    import numpy as np
    import torch
    from PIL import Image

    model = load_model(model_path)
    atlases = {}
    all_layers = [x for x in layers(imported) if not levels or x[2] in levels]
    for n, (k, ts, lvl, li, w, h, tiles) in enumerate(all_layers):
        if ts not in atlases:
            meta = json.load(open(os.path.join(imported, "tiles", ts + ".json"), encoding="utf-8"))
            atlases[ts] = (
                np.array(Image.open(os.path.join(imported, "tiles", ts + ".png")).convert("RGBA")),
                int(meta["columns"]),
            )
        atlas, cols = atlases[ts]
        img = compose(np, tiles, w, h, atlas, cols)
        rgb = img[..., :3]
        big, alpha_big = upscale_rgba(np, torch, model, img)
        dst = os.path.join(out_dir, "maps", k)
        os.makedirs(dst, exist_ok=True)
        chunks = []
        rows_used = [any(tiles[r * w : (r + 1) * w]) for r in range(h)]
        margin = 8 * K  # context of the grain rendering, discarded
        for c in range(0, -(-h // CHUNK_ROWS)):
            r0, r1 = c * CHUNK_ROWS, min(h, (c + 1) * CHUNK_ROWS)
            if not any(rows_used[r0:r1]):
                continue
            y0, y1 = r0 * TH * K, r1 * TH * K
            e0, e1 = y0 - BAND_MARGIN, y1 + BAND_MARGIN  # encoded band, margins included
            m0, m1 = max(0, e0 - margin), min(h * TH * K, e1 + margin)
            orig = np.repeat(np.repeat(rgb[m0 // K : m1 // K].astype(np.float32), K, axis=0), K, axis=1)
            part = restore_grain(np, torch, np.ascontiguousarray(big[m0:m1]), orig)
            part = part[max(0, e0) - m0 : min(h * TH * K, e1) - m0].round().astype(np.uint8)
            if alpha_big is not None:
                part = np.dstack([part, alpha_big[max(0, e0) : min(h * TH * K, e1)]])
            # Outside the map: repeated edge (never displayed, only encoded).
            part = np.pad(part, ((max(0, -e0), max(0, e1 - h * TH * K)), (0, 0), (0, 0)), mode="edge")
            Image.fromarray(part).save(
                os.path.join(dst, f"{c:03d}.webp"), "WEBP", quality=quality, method=4, exact=False
            )
            chunks.append(c)
        json.dump(
            {
                "tileset": ts,
                "level": lvl,
                "layer": li,
                "width": w,
                "height": h,
                "scale": K,
                "chunk_rows": CHUNK_ROWS,
                "margin": BAND_MARGIN,
                "chunks": chunks,
            },
            open(os.path.join(dst, "layer.json"), "w"),
        )
        print(
            f"hd_upscale : {n + 1}/{len(all_layers)} {lvl} layer {li + 1} ({w}×{h}) -> {k}, {len(chunks)} band(s)",
            flush=True,
        )
    write_manifest(
        out_dir,
        model="4x-PixelPerfectV4",
        model_sha256=MODEL_SHA256,
        scale=K,
        chunk_rows=CHUNK_ROWS,
        margin=BAND_MARGIN,
        webp_quality=quality,
        tool="hd_upscale.py render (spandrel + PyTorch, fp32, whole map per layer)",
    )


# --- Measurement ------------------------------------------------------------------


def border_jump(np, img, grid):
    """Two sums of |Δ|: at tile edges (both tiles exist) and at
    all the other x4 pixel edges of the image. The ratio of the means is
    the seam score: 1.00 = the tile edges cannot be told apart from the
    others. It depends on the content (art with sharp edges at the grid gives more
    than 1): compare the states with each other, not the score with 1."""
    a = img[..., :3].astype(np.float32)
    h, w = grid.shape
    tot = [0.0, 0.0]
    cnt = [0, 0]
    for axis in (0, 1):
        step = TH * K if axis == 0 else TW * K
        n_tiles = h if axis == 0 else w
        d = np.abs(np.diff(a, axis=axis)).mean(axis=2)
        border = np.zeros(d.shape[axis], bool)
        for k in range(1, n_tiles):
            border[k * step - 1] = True
        edges = np.zeros(d.shape[axis], bool)
        edges[3::4] = True
        for i in range(d.shape[axis]):
            if not edges[i]:
                continue
            if border[i]:
                k = (i + 1) // step
                if axis == 0:
                    m = np.repeat((grid[k - 1] > 0) & (grid[k] > 0), TW * K)
                    row = d[i]
                else:
                    m = np.repeat((grid[:, k - 1] > 0) & (grid[:, k] > 0), TH * K)
                    row = d[:, i]
                if not m.any():
                    continue
                tot[0] += float(row[m].sum())
                cnt[0] += int(m.sum())
            else:
                row = d[i] if axis == 0 else d[:, i]
                tot[1] += float(row.sum())
                cnt[1] += row.size
    return tot[0], cnt[0], tot[1], cnt[1]


def ratio(np, img, grid):
    sb, nb, so, no = border_jump(np, img, grid)
    return sb / nb / (so / no)


def atlas_hd(np, imported, ts, tiles, w, h, cols, hd_dir):
    """Tile atlas of upscaled tiles (hd-assets-N) composed 1:1 from the x4 map."""
    from PIL import Image

    hd = np.array(Image.open(os.path.join(hd_dir, "tiles", ts + ".png")).convert("RGBA"))
    img = np.zeros((h * TH * K, w * TW * K, 4), np.uint8)
    for i, n in enumerate(tiles):
        if n:
            r, c = divmod(i, w)
            ax, ay = (n - 1) % cols * TW * K, (n - 1) // cols * TH * K
            img[r * TH * K : (r + 1) * TH * K, c * TW * K : (c + 1) * TW * K] = hd[ay : ay + TH * K, ax : ax + TW * K]
    return img


def assemble(np, Image, out_dir, k):
    """Whole x4 layer pasted back from its bands (empty ones: transparent)."""
    meta = json.load(open(os.path.join(out_dir, "maps", k, "layer.json")))
    w, h = meta["width"], meta["height"]
    img = np.zeros((h * TH * K, w * TW * K, 4), np.uint8)
    mg = meta.get("margin", 0)
    for c in meta["chunks"]:
        part = np.array(Image.open(os.path.join(out_dir, "maps", k, f"{c:03d}.webp")).convert("RGBA"))
        part = part[mg : part.shape[0] - mg]
        y = c * CHUNK_ROWS * TH * K
        img[y : y + part.shape[0]] = part
    return img


def measure(imported, out_dir, levels):
    import numpy as np
    from PIL import Image

    Image.MAX_IMAGE_PIXELS = None
    for k, _ts, lvl, li, w, h, tiles in layers(imported):
        if levels and lvl not in levels:
            continue
        grid = np.array(tiles, np.int32).reshape(h, w)
        print(f"{lvl} layer {li + 1}: seam score {ratio(np, assemble(np, Image, out_dir, k), grid):.3f}")


def verify(imported, model_path, out_dir, levels, hd_dir=""):
    """Seam table per layer: original art, HD atlas (if provided),
    whole model image (reference without cutting), + grain, final bands."""
    import numpy as np
    import torch
    from PIL import Image

    Image.MAX_IMAGE_PIXELS = None
    model = load_model(model_path)
    head = "| level (layer) | original ×4 | HD atlas | model, whole map | + grain | WebP bands |"
    print(head + "\n|---|---|---|---|---|---|", flush=True)
    for k, ts, lvl, li, w, h, tiles in layers(imported):
        if lvl not in levels:
            continue
        meta = json.load(open(os.path.join(imported, "tiles", ts + ".json"), encoding="utf-8"))
        cols = int(meta["columns"])
        grid = np.array(tiles, np.int32).reshape(h, w)
        atlas = np.array(Image.open(os.path.join(imported, "tiles", ts + ".png")).convert("RGBA"))
        plain = compose(np, tiles, w, h, atlas, cols)
        r_orig = ratio(np, np.kron(plain, np.ones((K, K, 1), np.uint8)), grid)
        r_atlas = ratio(np, atlas_hd(np, imported, ts, tiles, w, h, cols, hd_dir), grid) if hd_dir else float("nan")
        rgb = plain[..., :3]
        big, _ = upscale_rgba(np, torch, model, plain)
        r_raw = ratio(np, big, grid)
        orig = np.repeat(np.repeat(rgb.astype(np.float32), K, axis=0), K, axis=1)
        grain = restore_grain(np, torch, np.ascontiguousarray(big), orig)
        r_grain = ratio(np, grain, grid)
        r_band = ratio(np, assemble(np, Image, out_dir, k), grid)
        print(
            f"| {lvl} L{li + 1} | {r_orig:.3f} | {r_atlas:.3f} | {r_raw:.3f} | {r_grain:.3f} | {r_band:.3f} |",
            flush=True,
        )
    if not hd_dir:
        print(
            "(HD atlas not provided: pass the folder of an hd-assets-N archive as the 5th argument)",
            flush=True,
        )


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else ""
    if cmd == "sprites" and len(sys.argv) >= 5:
        sprites(*sys.argv[2:5], sys.argv[5:])
    elif cmd == "interface" and len(sys.argv) == 5:
        interface(*sys.argv[2:5])
    elif cmd == "sharp" and len(sys.argv) >= 4:
        sharp_sprites(sys.argv[2], sys.argv[3], sys.argv[4:] or SHARP_SHEETS)
    elif cmd == "frames" and len(sys.argv) == 4:
        frames(sys.argv[2], sys.argv[3])
    elif cmd == "fidelity" and len(sys.argv) >= 5:
        fidelity(sys.argv[2], sys.argv[3], sys.argv[4:])
    elif cmd == "render" and len(sys.argv) >= 5:
        args = sys.argv[5:]
        q = WEBP_QUALITY
        if len(args) >= 2 and args[0] == "--quality":
            q, args = int(args[1]), args[2:]
        render(*sys.argv[2:5], args, q)
    elif cmd == "key" and len(sys.argv) == 3:
        for k, _ts, lvl, li, *_ in layers(sys.argv[2]):
            print(k, lvl, li)
    elif cmd == "digest" and len(sys.argv) == 3:
        print(digest(sys.argv[2]))
    elif cmd == "verify" and len(sys.argv) >= 6:
        rest, hd_dir = sys.argv[5:], ""
        if "--atlas" in rest:
            i = rest.index("--atlas")
            hd_dir = rest[i + 1]
            rest = rest[:i] + rest[i + 2 :]
        verify(sys.argv[2], sys.argv[3], sys.argv[4], rest, hd_dir)
    elif cmd == "measure" and len(sys.argv) >= 4:
        measure(sys.argv[2], sys.argv[3], sys.argv[4:])
    else:
        sys.exit(__doc__)
