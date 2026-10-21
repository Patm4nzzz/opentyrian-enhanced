# tyrian-formats

**Byte-exact** reading and writing of the original Tyrian 2000 data formats,
with no dependency on Godot. Reference: `docs/formats-spec.md`; when in doubt,
the C code in `src/` is authoritative.

## Modules

| Module | Files | C source |
|---|---|---|
| `crypt` | encrypted Pascal strings, `cubetxtN.dat`, `tyrian.cdt` | `helptext.c` |
| `container` | offset table `u16 + u32[]` | `lvllib.c`, `picload.c`… |
| `lvl` | `tyrianN.lvl`: header, enemies, events, `mapSh`, 3 layers | `level_load.c:JE_loadMap` |
| `episode` | `levelsN.dat`: raw lines, sections, `]L[`, `]I[` | `level_load.c` |
| `hdt` | `tyrian.hdt` + object block of episodes 4–5 | `episodes.c:JE_loadItemDat` |
| `sprite` | simple tables, Sprite2 sheets, `tyrian(c).shp`, `newsh$.shp` | `sprite.c`, `editship.c` |
| `tiles` | `shapesX.dat` (600 tiles 24×28) | `level_load.c` |
| `palette` | `palette.dat` (24 VGA 6-bit palettes) | `palette.c` |
| `pic` | `tyrian.pic`, PCX (`tshp2.pcx`, `shipedit.pcx`) | `picload.c`, `pcxload.c` |
| `snd` | `tyrian.snd`, `voices.snd`, `voicesc.snd` (+ WAV) | `nortsong.c` |
| `music` | `music.mus` (LDS) | `lds_play.c` |
| `demo` | `demo.1`…`demo.5` | `mainint.c` |
| `misc` | `tyrend.anm` (LPF structure), `exitmsg.bin` | `animlib.c` |

Each format keeps the bytes it does not read (tails, unused entries,
`mapSh[72..127]`, "bad data" voices…) so that `write(parse(b)) == b`.

## Tests

```sh
cargo test -p tyrian-formats -- --nocapture
```

Result: **80 files** of `data/` round-trip identically, 0 failures. Decoding checks: 18/12/12/20/8 levels,
`maxEvent` ≤ 2,500 (max 2,259), 677 help strings in `tyrian.hdt`, object
names, `]L[` sections, 304 sprites per Sprite2 sheet, 14 images × 64,000
pixels, 31/9 sounds.

Not covered (outside the original format, or format not established):
`sprite_manifest.json` (Enhanced JSON), `soundfont.sf2` (third-party),
`user1.shp`, `user2.shp` (unreferenced, format not established), and the
`midi/`, `locales/`, `reference/`
subdirectories (Enhanced resources).

## Import into Godot

```sh
cargo run --release -p tyrian-import -- --data ../data --out /tmp/ot-import
```

Produces `palettes.json`, `tiles/`, `sprites/`, `pics/`, `levels/`,
`episodes/`, `items.json`, `sfx/` (322 files, deterministic output).
