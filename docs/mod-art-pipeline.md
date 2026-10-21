# Mod art pipeline

How the art and the balance of a content pack in `rewrite/mods/` are made, so that
the next packs reach the same quality bar for the same cost. Worked example: the
Alien Weapon Pack (`rewrite/mods/alien_weapon_pack/`, October 2026); the quality
reference chosen for the packs is `rewrite/mods/alien_ship_pack/`.

The short version:

1. Reference sheet of the original sprites of the same kind.
2. Drafts with PixelLab `create_image_pixflux` at twice the final size, forced
   palette (one generation each, about $0.007).
3. Cleanup with `utils/mod_art_clean.py` (fit, palette snap, rim, shadow):
   ships, and icons when drafts are used. Weapon icons that must sit next to
   the originals are better built from the original housings (section 12).
   Shots are drawn by `utils/mod_shot_draw.py`, not generated.
4. Balance with `utils/mod_weapon_balance.py` against the originals of the
   same price, eleven power levels.
5. Checks: `mod_art_clean.py check`, the mod checker, `utils/mod_preview.py`,
   in-game captures (shop and play).

Tools: Python 3 with Pillow (`utils/requirements.txt`), WSL for the mod checker
and the captures (`docs/build.md`), the PixelLab MCP tools
`create_image_pixflux`, `create_image_pro`, `get_image`, `wait_for_jobs`,
`get_balance`. All cleanup is done by the scripts below, never by PixelLab's
own editing tools.

## 1. Sizes and formats

| Sprite | Folder | Size | Notes |
|---|---|---|---|
| Shop icon (weapon, shield...) | `sprites/pickups/weapons/` | 24x28 | grey housing, coloured core, near-black rim (palette index 16), two-step drop shadow down-right (indices 64 then 161) |
| Ship | `sprites/ships/` | 24x28 | five poses through `ship_poses`: `neutral`, `left`, `left_full`, `right`, `right_full` (roll: the cockpit slides toward the turn, the lowered wing shortens and darkens); dark outline, no drop shadow; the mods' ship image table holds five poses for each of the 14 mod ship slots (MODDING.md §5.5, ADR 0134) |
| Shot | `sprites/shots/` | 12x14 | near-white core, saturated body, dark rim of the same hue, no black outline; frames `<name>_1.png` .. `<name>_N.png` |

Animated shots: the engine numbers the PNGs of a sprite folder in name order
(`tyrian-mods/src/folder.rs`, `resolve/sprites.rs`), and a shot draws sprite
`slot + frame`, `frame` cycling from 0 to `animation` (one step per tick,
`tyrian-sim/src/shots.rs`). So the frames of one shot must sort together
(`ember_shell_1..4`, then `ember_shell_large_1..4`), the level cites `_1`, and
`"animation": N-1`. Every sprite used by one power level must have the same
number of frames, since `animation` belongs to the level.

## 2. Reference sheet

Original templates: `data/reference/sprites/` (shop icons in
`pickups/weapons/`, shots in `shots/`, ships in `ships/`).

```
python utils/mod_art_refsheet.py data/reference/sprites/pickups/weapons --scale 4 --cols 12 \
    --title "Original shop icons" --out <scratch>/refs/orig_icons_4x.png
```

Look at it before writing prompts: what makes the original icons is the
metal-grey body with hard white highlights, one saturated core, the rim and the
drop shadow. The player shots that the original weapons fire are listed by
the `sg` fields of `data/tyrian.hdt`; a sheet of them:

```
python utils/mod_art_refsheet.py "data/reference/sprites/shots/*.png" --scale 3 --cols 20 --no-labels --out <scratch>/refs/shots.png
```

The same tool compares the result with the originals at the end (section 7).

## 3. Palette

Everything is drawn in palette 5 of `data/palette.dat` (the gameplay palette,
`PCX_PALETTE[2]`). It is made of 16 ramps of 16 colours; print them with

```
python utils/mod_art_clean.py ramps
```

| Ramp | Colours | Used for |
|---|---|---|
| 0 | grey, black to white (index 0 is transparent, never used) | metal housings |
| 1 / 11, 10 / 14 | tan, sand, bone | bases, Hazudra bone |
| 2 | dark olive to pale bone | Hazudra organic parts |
| 4 | dark red | Zica heat vents |
| 7 | fire: dark red, red, orange, yellow, white | Zica cores and shots |
| 12 | neon green | Hazudra cores and shots |

Faction choice for the drafts and the cleanup (`--ramps`, `--glow`):

| Faction | Ramps | Glow ramp |
|---|---|---|
| Hazudra | 0, 10, 2 | 12 |
| Zica | 0, 7, 4 | 7 |

For PixelLab, a one-pixel-high PNG of those colours is the forced palette
(`color_image_base64` of `create_image_pixflux`): grey 1-15 + 112-127 +
66-75 + 161-171 for the Zica, grey + 32-46 + 192-207 + 161-173 for the
Hazudra. Building it:

```python
from PIL import Image
import sys

sys.path.insert(0, "utils")
from _shared import load_vga_palette

pal = load_vga_palette("data", 5)
idx = list(range(1, 16)) + list(range(112, 128)) + list(range(66, 76)) + list(range(161, 172))
im = Image.new("RGB", (len(idx), 1))
for x, i in enumerate(idx):
    im.putpixel((x, 0), pal[i])
im.save("pal_zica.png")
```

## 4. Drafts with PixelLab

### What worked: `create_image_pixflux` at 48x56

One generation per call (about $0.007 in paid credits), 20 to 90 seconds, one
job at a time on the free plan. Settings:

```
width 48, height 56 (2x an icon), no_background true,
outline "single color black outline", shading "medium shading",
detail "medium detail", view "high top-down",
color_image_base64 = the faction palette PNG above
```

Prompt pattern (the parts in brackets change):

> shop icon of a [heavy sci-fi mortar gun] seen from directly above, 1995 DOS
> shoot-em-up Tyrian style, symmetrical. [Gunmetal grey armoured body with
> thick side plates, one big round barrel opening at the top center glowing
> bright yellow orange and red like a furnace], black outline, hard highlights
> from upper left, centered, fills the canvas

What the six icons of the weapon pack's first version (1.0) used (the kept
draft, then the cleanup command of section 5). Version 2.0 replaced these icons
by faction variants of the original shop housings (section 12): the table is
kept as a record of what pixflux gives for this subject.

| Icon (1.0) | Subject of the prompt | Draft |
|---|---|---|
| Zica Thermal Lance | gun turret, long central lance barrel, white-hot core, red heat vents | 1st try |
| Hazudra Crescent Fan | spread gun, five curved bone crescent blades fanning out, big neon-green gem core | 2nd try (first core too dull) |
| Hazudra Spore Cannon | heavy cannon, big round glowing green spore pod in a grey cradle, bone ribs like claws | 1st try |
| Hazudra Seeker Darts | homing missile pod, three slim bone darts with neon-green tips | 2nd try (two darts read as a single pylon) |
| Zica Plasma Halo | ring emitter, octagonal housing, white-hot ring around a dark centre, four prongs | 2nd try (first ring too dark) |
| Zica Ember Mortar | heavy mortar from directly above, furnace-like barrel opening | 2nd try (first one was drawn in perspective) |

Lessons:

- Say "seen from directly above" and "symmetrical": without them pixflux draws
  three-quarter views.
- "brightly glowing neon green" / "white-hot yellow ... fading to orange and red"
  for the core; "glowing green" alone gives a dull olive core once the palette
  is forced.
- The draft's black outline is removed by the cleanup, which redraws the
  original rim: keep `outline "single color black outline"` anyway, it gives
  cleaner silhouettes.
- Generate two or three drafts per item and pick; at $0.007 a draft is cheaper
  than fixing a bad one.

### What did not work

- `create_image_pro` at 32x32 (10 generations per call): it renders one
  256x256 sheet cut into 64 tiles of 32x32. With a style image that was itself
  a grid of icons, the sheet did not follow the tile grid: every "candidate"
  was the bottom half of one icon over the top half of the next, unusable.
  `get_image` also returns only the first 4 of the 64 tiles inline. If pro is
  used again: a single icon as style image (never a sheet), and a canvas of
  64 px or more (16 candidates) so that one candidate holds one whole subject.
- Generating shots: at 12x14 every generator blurs the core and the rim. The
  shots are drawn by script (section 6).

### Budget record

| Pack | Free generations | Paid | What |
|---|---|---|---|
| Alien Weapon Pack | 20 (two `create_image_pro` calls, unusable) | $0.08 (12 pixflux drafts) | balance $17.20 -> $17.12 |

## 5. Cleanup: `utils/mod_art_clean.py`

`icon` crops the draft to its subject, box-downscales it to fit 24x28 minus the
rim and shadow, raises the contrast the downscale flattens, snaps every pixel to
the chosen ramps (saturated pixels go to the glow ramp, so the core stays
bright), removes the draft's own black edge, then draws the near-black rim
(index 16) and the two-step drop shadow (64, 161).

Commands used for the weapon pack's first icons (version 1.0, drafts kept
outside the repository; the 2.0 icons come from section 12):

```
o="--contrast 1.4 --lift 1"
python utils/mod_art_clean.py icon pf_lance_1.png  .../zica_thermal_lance_icon.png   --ramps 0,7,4  --glow 7  $o
python utils/mod_art_clean.py icon pf_halo_2.png   .../zica_plasma_halo_icon.png     --ramps 0,7,4  --glow 7  $o
python utils/mod_art_clean.py icon pf_mortar_2.png .../zica_ember_mortar_icon.png    --ramps 0,7,4  --glow 7  $o
python utils/mod_art_clean.py icon pf_fan_2.png    .../hazudra_crescent_fan_icon.png --ramps 0,10,2 --glow 12 $o
python utils/mod_art_clean.py icon pf_spore_1.png  .../hazudra_spore_cannon_icon.png --ramps 0,10,2 --glow 12 --glow-sat 0.4 $o
python utils/mod_art_clean.py icon pf_darts_2.png  .../hazudra_seeker_darts_icon.png --ramps 0,10,2 --glow 12 $o
```

Tuning:

- `--contrast 1.4 --lift 1`: without them the icons come out darker and flatter
  than the originals (the box filter averages the highlights away).
- `--glow-sat`: raise it (0.4 to 0.6) when bone or metal pixels of the draft turn
  into green or orange specks; lower it (0.25) when the core is too pale to be
  caught.
- Ships: same command with `--size 24x28 --no-shadow` on a top-down ship draft,
  then the four banking poses drawn from the neutral one (shift the cockpit one
  pixel toward the turn, shorten and darken the lowered wing), as in
  `alien_ship_pack`.

Compare at 1x and 2x with the originals before going on:

```
python utils/mod_art_refsheet.py rewrite/mods/<pack>/sprites/pickups/weapons \
    data/reference/sprites/pickups/weapons/zica_laser.png data/reference/sprites/pickups/weapons/protron.png \
    --scale 2 --cols 12 --out <scratch>/icons_vs_orig.png
```

## 6. Shots: `utils/mod_shot_draw.py`

Shapes (`lance`, `orb`, `crescent`, `dart`, `ring`, `ember`) are intensity
functions sampled 4x4 per pixel; the intensity picks a step of a palette
gradient (`fire`, `green`, `bone`); edge pixels take the darkest steps (the dark
rim). Four frames vary the shape (flicker, pulse, spin). The weapon pack's
shots, exactly:

```
D=rewrite/mods/alien_weapon_pack/sprites/shots
python utils/mod_shot_draw.py lance    fire  $D/thermal_lance
python utils/mod_shot_draw.py crescent green $D/hazudra_crescent
python utils/mod_shot_draw.py orb      green $D/spore_pod --size 7
python utils/mod_shot_draw.py orb      green $D/spore_pod_large --size 11
python utils/mod_shot_draw.py dart     green $D/seeker_dart
python utils/mod_shot_draw.py ring     fire  $D/plasma_ring
python utils/mod_shot_draw.py ember    fire  $D/ember_shell --size 7
python utils/mod_shot_draw.py ember    fire  $D/ember_shell_large --size 10
```

Rules of thumb: a shot must fill most of its 12x14 cell to be seen in play
(the darts were enlarged after the first in-game captures); give a heavy
weapon a larger sprite for its big hits (`spore_pod_large`,
`ember_shell_large`); use the faction gradient, never a colour of the other
faction.

## 7. Balance: `utils/mod_weapon_balance.py`

Formula (same as the wiki): DPS = damage of one volley × 35 / (fire_delay + 1);
energy per second = power_use × 35 / (fire_delay + 1).

```
python utils/mod_weapon_balance.py --list-originals                  # every original, 11 levels
python utils/mod_weapon_balance.py rewrite/mods/<pack>/mod.json --names "Zica Laser,HotDog Rear,..." --median
python utils/mod_weapon_balance.py rewrite/mods/<pack>/mod.json --names "..." --median --energy
```

Method:

1. Pick the direct-fire originals of the same price band (±100 credits); the
   missile and bomb launchers count their chain shots as 1 and are not
   comparable.
2. Take the median per level as the target curve, then set each weapon by role:
   piercing and homing 0-30 % under the median, spread and area at the median
   (counting every projectile), heavy up to +15 % with the highest power use.
   Never above the strongest original of the price at any level.
3. Set `power_use` so that energy per second sits in the range of the same
   originals (it is paid per volley, so a slow gun needs a high `power_use`).
4. Eleven levels per weapon (the format allows up to 11, like the originals),
   at most 8 projectiles per level.
5. Put both tables (DPS, energy) in the pack README.

The weapon pack's `mod.json` was written by a small generator kept outside the
repository (level tables in Python, DPS printed while tuning); editing the
JSON5 by hand is fine too, then rerun the helper.

## 8. Checks

```
python utils/mod_art_clean.py check rewrite/mods/<pack>/sprites     # palette, alpha, sizes: 0 problems
```

Mod checker, under WSL (`docs/build.md`); a Linux copy of the repository builds
much faster than a Windows drive mounted under `/mnt`:

```
rsync -a --delete <repo>/rewrite/mods/ <copy>/rewrite/mods/
cd <copy>/rewrite && CARGO_TARGET_DIR=<target> cargo run --release -p tyrian-mods -- check --data ../data --enable-all
```

Expected: `0 error(s), 0 warning(s)`.

## 9. Preview

```
python utils/mod_preview.py rewrite/mods/<pack> "<Pack name>"
```

writes `preview.png` (1x) and `preview_3x.png` in the pack folder: title, one
row per category (ships without their poses, shop icons, shots with their
first frame, then every shot frame). Look at `preview_3x.png` next to
`rewrite/mods/alien_ship_pack/preview_3x.png`: same background, the sprites
must hold up at that scale.

## 10. In-game captures

Both from a WSL copy where `scripts/build-linux.sh` has run (the Godot binary,
`bin/`, the imported assets).

Shop (capture tool `rewrite/godot/tools/capture.gd`): `--window` is required,
the mod icons are drawn on a layer above the 8-bit framebuffer and a plain
capture misses them.

```
cd <copy>/rewrite/godot
xvfb-run -a -s "-screen 0 1280x800x24" godot --rendering-driver opengl3 --path . -- \
  --mods=<pack> --capture="shop:<out>/shop_front.png:40" --window \
  --shop-loadout=1,1,1,0,1,4,6,0,0,0,500000 \
  --keys=Down,Down,Enter,Down,Enter,Down,Down,Down,Down
```

`Down,Down,Enter` opens Upgrade Ship, then `Down,Enter` the front guns
(`Down,Down,Enter` the rear guns); the mod weapons come after the originals;
`Right` raises the power level shown.

Play (movie writer + `rewrite/godot/tools/trailer_clip.gd`, live mode): the
loadout is `ship, front weapon, front power, rear weapon, rear power, shield,
generator, left sidekick, right sidekick, special, cash`; the mod weapons take
item ids from 61 in `mod.json` order when only that pack is enabled.

```
godot --rendering-driver opengl3 --path . --write-movie <out>/f.png --fixed-fps 35 --resolution 960x600 \
  res://tools/trailer_clip.tscn -- --tc-size=960x600 --tc-mute --mods=<pack> --tc-mode=live \
  --tc-level=9 --tc-from=30 --tc-frames=90 --tc-loadout=1,61,1,0,1,4,6,0,0,0,0 --tc-dmg=0
```

Live mode turns on the autopilot helpers, which also raise both guns to power
11: a capture at power 1 needs them off. `trailer_clip.gd` has no option for
that; for the weapon pack a throw-away patch of the WSL copy read
`--tc-assist=0` (`Capture.replay_assist = String(_opts.get("assist", "1")) == "1"`).
Then pick, in each clip, the frame that shows the most pixels of the shot's
colours (the weapon pack's selection script did exactly that) and lay the
clips out power 1 / power 11 side by side.

Known limit: the shop's firing preview draws the 8-bit framebuffer only, so
it shows no mod shot sprites (true of every mod with its own shot PNGs); judge the
shots from the play captures.

## 11. Order of work for a new pack

1. Read MODDING.md §5-6, the pack's current `mod.json` and README.
2. `get_balance`; reference sheets; palette PNGs.
3. Drafts (pixflux 48x56, 2-3 per item), pick, cleanup, compare with the
   originals; shots by script.
4. Balance tables, `mod.json` (keep `$schema`, `format_version` 1,
   `"enabled": false`), README.
5. Checks, preview, captures; fix what reads badly in play (size, colour) and
   redo the affected steps.
6. `get_balance`; record the cost in the table of section 4.

## 12. Icons: faction variants of the original housings

The first weapon-pack icons (pixflux drafts, section 4) came out round and
simple, halfway between a ship and a gem, and the Plasma Halo read as a
pickup. The original shop icons are gun housings: boxy metal bodies, dense
detail, the near-black rim, the two-step drop shadow, one coloured core or
barrel. The six icons were therefore rebuilt from parts of the originals by
`utils/alien_weapon_icons.py` (no generation, no credits):

```
python utils/alien_weapon_icons.py --orig <website>/site/wiki/assets/sprites/items \
    --out rewrite/mods/alien_weapon_pack/sprites/pickups/weapons
```

It reads the wiki extracts `weapon-NN-*.png` (the icon cropped and enlarged
2x, reduced back to 1x), removes their shadow, pastes cropped parts on a
24x28 canvas, recolours them, draws the cores (shaded discs and rings lit
from the upper left), closes the rim (16) and redraws the shadow (64, 161).

| Icon | Parts | Faction look |
|---|---|---|
| Zica Thermal Lance | missile-launcher barrel on the rear-vulcan housing | white-hot barrel, hot metal, orange side vents |
| Zica Plasma Halo | fireball-gun horns over the laser's bar housing | white-hot ring with a dark bore between the horns |
| Zica Ember Mortar | scatter-wave twin housing | fat grey muzzle, dark bore, ember core; fire vents |
| Hazudra Crescent Fan | starburst arms on the pulse-cannon base | bone arms, olive carapace base, green core |
| Hazudra Spore Cannon | pulse-cannon twin barrels on mega-cannon legs | bone barrels, grey legs, large green spore pod |
| Hazudra Seeker Darts | three missile-launcher darts on the laser's bar housing | bone darts, green warheads, carapace base |

Rules that made them pass next to the originals:

- Mix at least two originals per icon, and never keep the full silhouette of a
  well-known weapon (the Zica Laser, the Protron and the Mega Cannon box were
  avoided).
- Zica "hot metal": only the three darkest greys go to the dark red ramp 4
  (65-68); recolouring more greys makes the housing look rusty.
- Hazudra plating: grey to ramp 2 (olive to pale bone) for the bodies, to
  ramp 10 for bone barrels and arms; cores in ramp 12.
- Two icons of the same faction must differ by their base too (the Spore
  Cannon keeps grey legs so it does not read as the Crescent Fan in the list).
- Judge at 1x and 3x next to the originals (section 5 command) and in the
  shop capture (section 10): the list shows five icons at once, the pack's
  icons must not stand out from the original ones around them.
