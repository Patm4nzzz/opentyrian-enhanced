#!/usr/bin/env python3
"""asset_audit.py — drift detector for reference sprites ↔ sprite_manifest.

After running `asset_build_hd_png.py`, `asset_extract_*.py`, or hand-
editing `data/sprite_manifest.json`, the reference PNGs under
`data/reference/sprites/<category>/` and the manifest are easy to get
out of sync. This tool reports every kind of mismatch without
touching the filesystem:

  1. **Missing PNGs** — manifest entries whose file doesn't exist.
     Causes sprite-name lookups to fail silently in mods that
     reference the entry.
  2. **Orphan PNGs** — files on disk that aren't in the manifest.
     They still load via numeric filename (e.g. `123.png`), but named
     refs from mod.json can't resolve them.
  3. **Index collisions** — two manifest entries point at the same
     sprite index inside a section. Last writer wins at runtime → the
     "other" name silently returns a different sprite.
  4. **Size mismatches** — the PNG's real pixel dimensions disagree
     with the `frame_width` / `frame_height` the manifest advertises.
     Causes clipped or scrambled blits in-game.
  5. **Bad PNGs** — files that don't start with the PNG magic header.

Exit code 1 when any error is found, 0 otherwise. Warnings alone
(orphans, minor size drift) don't fail the exit code unless `--strict`.

Usage:
    python3 utils/asset_audit.py                  # full report
    python3 utils/asset_audit.py --category ships # only one section
    python3 utils/asset_audit.py --strict         # fail on warnings too
    python3 utils/asset_audit.py --json           # machine-readable
"""

from __future__ import annotations

import argparse
import json
import os
import struct
import sys
import zlib as _zlib
from collections import defaultdict
from pathlib import Path
from typing import Any

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from _shared import repo_root  # noqa: E402
from _sprite_manifest import load_manifest  # noqa: E402

# Categories that live under data/reference/sprites/ AND have a matching
# section in sprite_manifest.json. Categories like "destruct", "ui",
# "levels", "weapon_effects" are reference-only and deliberately lack a
# manifest — they're not moddable, so we skip them.
CATEGORIES = (
    "ships",
    "shots",
    "sidekicks",
    "pickups",
    "coins",
    "weapon_icons",
    "explosions",
    "planets",
    "portraits",
)


def png_dimensions(path: Path) -> tuple[int, int] | None:
    """Parse a PNG IHDR to extract (width, height). Returns None on
    any error. Stdlib-only so this script has zero deps."""
    try:
        with path.open("rb") as f:
            sig = f.read(8)
            if sig != b"\x89PNG\r\n\x1a\n":
                return None
            # IHDR is the first chunk after the signature.
            # 4-byte length + 4-byte type ("IHDR") + 13-byte data.
            length = struct.unpack(">I", f.read(4))[0]
            ctype = f.read(4)
            if ctype != b"IHDR" or length < 8:
                return None
            w = struct.unpack(">I", f.read(4))[0]
            h = struct.unpack(">I", f.read(4))[0]
            return w, h
    except (OSError, struct.error):
        return None


def _manifest_entry(value: Any) -> tuple[int | None, int | None, int | None, int]:
    """Normalize a manifest value to (index, frame_width, frame_height, frames).
    Values can be bare ints (legacy) or dicts. Missing fields → None / 1."""
    if isinstance(value, int):
        return value, None, None, 1
    if isinstance(value, dict):
        idx = value.get("index") if isinstance(value.get("index"), int) else None
        fw = value.get("frame_width") if isinstance(value.get("frame_width"), int) else None
        fh = value.get("frame_height") if isinstance(value.get("frame_height"), int) else None
        fr = value.get("frames") if isinstance(value.get("frames"), int) else 1
        return idx, fw, fh, max(fr, 1)
    return None, None, None, 1


def audit_category(category: str, manifest_section: dict[str, Any], root: Path) -> dict[str, list[str]]:
    """Audit one manifest section. Returns {errors, warnings}."""
    errors: list[str] = []
    warnings: list[str] = []

    # Step 1: discover every PNG under the category root, keyed by the
    # relative path (without extension) from the category root. Manifest
    # keys like "weapons/vulcan_cannon" can then match directly.
    category_root = root / "data" / "reference" / "sprites" / category
    on_disk: dict[str, Path] = {}
    if category_root.is_dir():
        for png in category_root.rglob("*.png"):
            rel = png.relative_to(category_root).with_suffix("")
            on_disk[str(rel).replace(os.sep, "/")] = png

    # Step 2: cross-check each manifest entry. Aliases are tolerated:
    # when two names share the exact same (index, fw, fh), we treat the
    # later entry as an alias of the first — no error.
    seen_indices: dict[int, tuple[str, tuple[int | None, int | None]]] = {}
    # Animation-range occupancy: maps every table index an entry will
    # WRITE to (after the C loader slices its strip) back to the entry
    # name. 12px sprites slice at stride 1, 24px composites at stride 2
    # — exactly mirroring sprite_override.c. Overlaps mean one entry
    # stamps its frames over another\'s animation range (the gem_green/
    # coin_pink class of bug).
    occupied: dict[int, str] = {}
    manifest_names: set = set()
    for name, value in manifest_section.items():
        manifest_names.add(name)
        idx, fw, fh, frames = _manifest_entry(value)

        if idx is None:
            errors.append(f"{category}/{name}: manifest entry has no numeric 'index'")
            continue

        # Collision detection with alias tolerance.
        is_alias = False
        prev = seen_indices.get(idx)
        if prev is not None:
            prev_name, prev_dims = prev
            if prev_dims == (fw, fh):
                # Same index AND same dims → intentional alias; fine.
                is_alias = True
            else:
                errors.append(
                    f"{category}: '{name}' and '{prev_name}' both map to "
                    f"index {idx} with different dims {prev_dims} vs {(fw, fh)} "
                    f"— runtime lookup is non-deterministic"
                )
        else:
            seen_indices[idx] = (name, (fw, fh))

        # Occupancy: indices this entry\'s sliced frames will write to.
        # Aliases (same base index + dims as an earlier entry) occupy an
        # identical range by construction — skip them.
        if fw is not None and not is_alias:
            stride = 2 if fw >= 24 else 1
            for f in range(frames):
                t = idx + f * stride
                prev_owner = occupied.get(t)
                if prev_owner is not None and prev_owner != name:
                    # Warning, not error: stock data legitimately shares
                    # transition frames between color-cycling entries
                    # (identical content at the shared index). A REAL
                    # bleed (different content) needs --content to prove.
                    warnings.append(
                        f"{category}: '{name}' frame {f} writes to index {t} "
                        f"already owned by '{prev_owner}' — animation ranges "
                        f"overlap (verify the shared frame is identical)"
                    )
                else:
                    occupied[t] = name

        # Missing PNG — names with a slash point into a subfolder, names
        # without match a top-level file in the category root.
        path = on_disk.get(name)
        if path is None:
            warnings.append(
                f"{category}/{name}: manifest entry index={idx} has no PNG at "
                f"data/reference/sprites/{category}/{name}.png"
            )
            continue

        # Bad / truncated PNG
        dims = png_dimensions(path)
        if dims is None:
            errors.append(f"{path}: invalid PNG header or truncated IHDR")
            continue

        # Dimension mismatch (only when manifest declares both)
        if fw is not None and fh is not None:
            real_w, real_h = dims
            ok = (
                (real_w == fw and real_h == fh)
                or (real_w % fw == 0 and real_h == fh)
                or (real_h % fh == 0 and real_w == fw)
            )
            if not ok:
                warnings.append(f"{path}: PNG dims {real_w}×{real_h} don't match manifest frame {fw}×{fh}")
            # Multi-frame strips are sliced by the C loader: the PNG
            # width MUST be exactly frames × frame_width or the loader
            # refuses to slice and the whole strip renders at the base
            # index (the duplicated-coins bug class). Hard error.
            if frames > 1 and real_w != frames * fw:
                errors.append(
                    f"{path}: strip declares frames={frames} fw={fw} but "
                    f"width is {real_w} (expected {frames * fw}) — the "
                    f"loader will NOT slice this strip"
                )

    # Step 3: flag orphan PNGs — files on disk not in the manifest.
    # Purely numeric filenames resolve by index at runtime, so they're
    # not orphans — skip.
    for rel in sorted(on_disk.keys() - manifest_names):
        stem = rel.rsplit("/", 1)[-1]
        if stem.isdigit():
            continue
        warnings.append(
            f"{category}: orphan PNG '{rel}.png' not in manifest — mods can't reference by name (index-only)"
        )

    return {"errors": errors, "warnings": warnings}


# ── Content validation (--content) ─────────────────────────────────
# Compares every reference PNG's per-frame color signature against the
# BINARY sprite at the index the C loader will use (stride 1 for 12px,
# stride 2 for 24px composites). This is the check that would have
# caught the 2026-06 coin bugs: strip-at-base, wrong stride, and the
# cross-section name collision all produce content/index mismatches.
# Stdlib-only (zlib PNG inflate + the shared Sprite2 decoder).

# manifest section -> list of (tyrian.shp section, index offset).
# Indices > 500 route to the second sheet at offset 500.
_CONTENT_SHEETS = {
    "shots": [(7, 0), (11, 500)],
    "ships": [(8, 0), (12, 500)],
    "pickups": [(9, 0)],
    "coins": [(10, 0)],
}


def _read_png_rgba(path: Path):
    d = path.read_bytes()
    w, h = struct.unpack(">II", d[16:24])
    ct = d[25]
    if ct not in (2, 6):
        return None, w, h
    bpp = 4 if ct == 6 else 3
    idat = b""
    pos = 8
    while pos < len(d):
        ln, typ = struct.unpack(">I4s", d[pos : pos + 8])
        if typ == b"IDAT":
            idat += d[pos + 8 : pos + 8 + ln]
        pos += 12 + ln
    raw = _zlib.decompress(idat)
    stride = w * bpp
    out = bytearray(w * h * 4)
    prev = bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]
        line = bytearray(raw[p + 1 : p + 1 + stride])
        p += 1 + stride
        if f == 1:
            for i in range(bpp, stride):
                line[i] = (line[i] + line[i - bpp]) & 0xFF
        elif f == 2:
            for i in range(stride):
                line[i] = (line[i] + prev[i]) & 0xFF
        elif f == 3:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                line[i] = (line[i] + ((a + prev[i]) >> 1)) & 0xFF
        elif f == 4:
            for i in range(stride):
                a = line[i - bpp] if i >= bpp else 0
                b_ = prev[i]
                c = prev[i - bpp] if i >= bpp else 0
                pa, pb, pc = abs(b_ - c), abs(a - c), abs(a + b_ - 2 * c)
                pr = a if (pa <= pb and pa <= pc) else (b_ if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        prev = line
        for x in range(w):
            i4 = (y * w + x) * 4
            ib = x * bpp
            out[i4 : i4 + 3] = line[ib : ib + 3]
            out[i4 + 3] = line[ib + 3] if bpp == 4 else 255
    return bytes(out), w, h


def _png_sig(rgba, w, h, fx0, fw):
    rs = gs = bs = n = 0
    for y in range(h):
        for x in range(fx0, fx0 + fw):
            i = (y * w + x) * 4
            if rgba[i + 3] >= 128:
                rs += rgba[i]
                gs += rgba[i + 1]
                bs += rgba[i + 2]
                n += 1
    if n == 0:
        return None
    return (rs / n, gs / n, bs / n, n)


def audit_content(root: Path, only_category: str | None) -> dict[str, list[str]]:
    """Cross-check reference PNG content against the binary sheets."""
    sys.path.insert(0, str(root / "utils"))
    from _shared import decode_sprite2, load_shp_section, load_vga_palette

    warnings: list[str] = []
    errors: list[str] = []
    data_dir = str(root / "data")

    pal = load_vga_palette(data_dir)
    manifest = load_manifest()

    sheets: dict[int, tuple[bytes, list[int], int]] = {}

    def sheet(idx: int):
        if idx not in sheets:
            blob = load_shp_section(data_dir, idx)
            if not blob:
                sheets[idx] = (b"", [], 0)
            else:
                num = struct.unpack_from("<H", blob, 0)[0] // 2
                offs = [struct.unpack_from("<H", blob, 2 * i)[0] for i in range(num)]
                sheets[idx] = (blob, offs, num)
        return sheets[idx]

    def bin_sig(sheet_idx: int, game_idx: int):
        blob, offs, num = sheet(sheet_idx)
        b = game_idx - 1
        if b < 0 or b >= num:
            return None
        try:
            px = decode_sprite2(blob, offs[b])
        except Exception:
            return None
        cols = [c for row in px for c in row if c]
        if not cols:
            return None
        return (
            sum(pal[c][0] for c in cols) / len(cols),
            sum(pal[c][1] for c in cols) / len(cols),
            sum(pal[c][2] for c in cols) / len(cols),
            len(cols),
        )

    checked = 0
    for cat, routes in _CONTENT_SHEETS.items():
        if only_category and cat != only_category:
            continue
        section = manifest.get(cat)
        if not isinstance(section, dict):
            continue
        for name, value in section.items():
            if "/" in name:
                continue  # nested shop entries target newsh1.shp / sheet10 — out of scope here
            idx, fw, fh, frames = _manifest_entry(value)
            if idx is None or fw is None:
                continue
            png = root / "data" / "reference" / "sprites" / cat / f"{name}.png"
            if not png.is_file():
                continue
            try:
                rgba, w, h = _read_png_rgba(png)
            except Exception:
                errors.append(f"{cat}/{name}: unreadable PNG for content check")
                continue
            if rgba is None or w != frames * fw:
                continue  # palette PNG or strip-width issue (covered elsewhere)
            sheet_idx, base_off = routes[0]
            if len(routes) > 1 and idx > 500:
                sheet_idx, base_off = routes[1]
            stride = 2 if fw >= 24 else 1
            for f in range(frames):
                ps = _png_sig(rgba, w, h, f * fw, fw)
                target = idx - base_off + f * stride
                if fw >= 24:
                    cells = [target, target + 1, target + 19, target + 20]
                    sigs = [bin_sig(sheet_idx, c) for c in cells]
                    sigs = [sg for sg in sigs if sg]
                    bs_ = tuple(sum(sg[k] for sg in sigs) / len(sigs) for k in range(3)) if sigs else None
                else:
                    sg = bin_sig(sheet_idx, target)
                    bs_ = sg[:3] if sg else None
                if ps is None or bs_ is None:
                    continue  # custom range / empty sprite — not comparable
                checked += 1
                d = sum(abs(ps[k] - bs_[k]) for k in range(3))
                # Small sprites have noisy signatures — require a real
                # pixel population before judging.
                if d > 60 and ps[3] >= 20:
                    errors.append(
                        f"{cat}/{name} frame {f} @index {target}: content does not "
                        f"match the binary sprite (dRGB={d:.0f}) — wrong index, "
                        f"stride or extraction"
                    )
                elif d > 40 and ps[3] >= 20:
                    warnings.append(
                        f"{cat}/{name} frame {f} @index {target}: borderline "
                        f"content mismatch (dRGB={d:.0f}) — eyeball it"
                    )
    if checked == 0:
        warnings.append("content check compared 0 frames — data paths or manifest sections look wrong")
    else:
        print(f"content check: {checked} frames compared against binary sheets")
    return {"errors": errors, "warnings": warnings}


# Findings investigated and verified benign (2026-06 audit). They stay
# VISIBLE in the report (prefixed "ack:") but no longer fail --strict.
# Remove an entry here if the underlying data changes.
ACKNOWLEDGED_WARNINGS = (
    # Same name in two sections is legitimate since the C loader went
    # section-aware (gem_green incident, commit d5177770).
    "name 'bomb' exists in multiple sections",
    "name 'gem_green' exists in multiple sections",
    "name 'hotdog' exists in multiple sections",
    # datacube_green/datacube_green_gray share a color-cycle transition
    # frame at index 273 — contents verified identical by signature.
    "'datacube_green' frame 0 writes to index 273",
)


def _split_acknowledged(warnings: list[str]) -> tuple[list[str], list[str]]:
    live, acked = [], []
    for w in warnings:
        if any(pat in w for pat in ACKNOWLEDGED_WARNINGS):
            acked.append("ack: " + w)
        else:
            live.append(w)
    return live, acked


def audit(root: Path, only_category: str | None) -> dict[str, Any]:
    manifest = load_manifest()
    report: dict[str, Any] = {"categories": {}, "totals": {"errors": 0, "warnings": 0}}

    # Cross-section duplicate names. The same name in two sections at
    # different indices is legal NOW (the C loader is section-aware
    # since the gem_green incident), but it stays a modder footgun:
    # any tool resolving names without a section gets whichever section
    # came first. Surface them as warnings.
    name_homes: dict[str, list[tuple[str, Any]]] = defaultdict(list)
    for sec_name, sec in manifest.items():
        if not isinstance(sec, dict):
            continue
        for n, v in sec.items():
            idx, _, _, _ = _manifest_entry(v)
            name_homes[n].append((sec_name, idx))
    xsec = {
        n: homes for n, homes in name_homes.items() if len({h[0] for h in homes}) > 1 and len({h[1] for h in homes}) > 1
    }
    if xsec and not only_category:
        warns = [
            f"name '{n}' exists in multiple sections with different indices: " + ", ".join(f"{s}@{i}" for s, i in homes)
            for n, homes in sorted(xsec.items())
        ]
        live, acked = _split_acknowledged(warns)
        report["categories"]["_cross_section"] = {"errors": [], "warnings": live + acked}
        report["totals"]["warnings"] += len(live)

    for cat in CATEGORIES:
        if only_category and cat != only_category:
            continue
        section = manifest.get(cat)
        if not isinstance(section, dict):
            report["categories"][cat] = {
                "errors": [f"{cat}: manifest section missing or not a dict"],
                "warnings": [],
            }
            report["totals"]["errors"] += 1
            continue
        res = audit_category(cat, section, root)
        live, acked = _split_acknowledged(res["warnings"])
        res["warnings"] = live + acked
        report["categories"][cat] = res
        report["totals"]["errors"] += len(res["errors"])
        report["totals"]["warnings"] += len(live)

    return report


def format_text(report: dict[str, Any]) -> str:
    lines: list[str] = []
    for cat, res in report["categories"].items():
        errs = res["errors"]
        warns = res["warnings"]
        lines.append(f"== {cat} ==  errors={len(errs)} warnings={len(warns)}")
        for w in warns:
            lines.append(f"  WARN  {w}")
        for e in errs:
            lines.append(f"  ERROR {e}")
        if not errs and not warns:
            lines.append("  OK")
    t = report["totals"]
    lines.append(f"\nTotal: {t['errors']} error(s), {t['warnings']} warning(s)")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    p.add_argument("--category", choices=sorted(CATEGORIES), help="Audit only one category (default: all)")
    p.add_argument(
        "--content",
        action="store_true",
        help="also validate PNG content against the binary sheets (catches wrong index / stride / extraction bugs)",
    )
    p.add_argument("--strict", action="store_true", help="Fail the exit code on warnings as well as errors")
    p.add_argument("--json", dest="as_json", action="store_true", help="Emit machine-readable JSON")
    args = p.parse_args(argv)

    report = audit(repo_root(), args.category)
    if args.content:
        cres = audit_content(repo_root(), args.category)
        report["categories"]["_content"] = cres
        report["totals"]["errors"] += len(cres["errors"])
        report["totals"]["warnings"] += len(cres["warnings"])
    if args.as_json:
        print(json.dumps(report, indent=2))
    else:
        print(format_text(report))

    t = report["totals"]
    if t["errors"] > 0:
        return 1
    if args.strict and t["warnings"] > 0:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
