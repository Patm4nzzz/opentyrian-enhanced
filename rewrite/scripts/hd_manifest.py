#!/usr/bin/env python3
"""Completes <folder>/manifest.json with the counters of the HD archive (ADR 0100).

Called by hd_assets.sh pack: what the manifest announces must match
the bytes of the archive, otherwise a published package lies about its content.
"""

import json
import os
import sys

src = sys.argv[1]
path = os.path.join(src, "manifest.json")
m = json.load(open(path, encoding="utf-8"))
m["sprites"] = len([f for f in os.listdir(os.path.join(src, "sprites")) if f.endswith(".png")])
m["bands"] = sum(len([f for f in fs if f.endswith(".webp")]) for _, _, fs in os.walk(os.path.join(src, "maps")))
# Interface images: full-screen backgrounds, portraits and atlases in a screen
# palette (hd_upscale.py interface).
ui = (
    [os.path.join("pics", f) for f in os.listdir(os.path.join(src, "pics"))]
    if os.path.isdir(os.path.join(src, "pics"))
    else []
)
for sub in sorted(os.listdir(os.path.join(src, "sprites"))):
    if os.path.isdir(os.path.join(src, "sprites", sub)):
        ui += [os.path.join("sprites", sub, f) for f in os.listdir(os.path.join(src, "sprites", sub))]
m["interface_images"] = len([f for f in ui if f.endswith(".png")])
# Bytes of the images and fingerprints, without the manifest itself (its size depends
# on this count: two passes would give two values).
m["bytes"] = sum(
    os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(src) for f in fs if os.path.join(r, f) != path
)
json.dump(m, open(path, "w", encoding="utf-8"), ensure_ascii=False, indent=2, sort_keys=True)
print(
    f"hd_manifest: {m['sprites']} sprite(s), {m['interface_images']} interface image(s), {m['bands']} band(s), {m['bytes']} byte(s)"
)
