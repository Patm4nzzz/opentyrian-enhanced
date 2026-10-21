# Bio Ships Pack

Six living ships of a bio-organic alien faction: green carapaces, membrane
wings, eyes, tendrils and glowing spore sacs, with bone and flesh accents.
Still Tyrian ships seen from above, nose up.

Every sprite is drawn the way the original ships are, like the Alien Ship
Pack: 24x28 canvas, only the game's VGA palette colours (olive-green ramp 2,
bone ramp 10, flesh ramp 4, neon-green glow ramp 12), a one-pixel dark outline,
light from the upper left, no dithering. Each ship has five poses like the
originals: neutral, half bank and full bank to each side (the hull slides
toward the turn, the lowered wing shortens and darkens).

Enable it in **Setup → Mods**. All six ships are sold in every shop.

| Ship | Role | Price | Armor | Description | Original ships of the same tier (price / armor) |
|---|---|---|---|---|---|
| Spore Wisp | light interceptor | 8500 | 13 | Small winged seed around one big glowing spore sac. | USP Fang 8000 / 14; Stalker 10000 / 15 |
| Thornwing | agile striker | 11000 | 16 | Moth-shaped body with swept membrane wings tipped by bone thorns, thorn legs and a glowing green heart. | Stalker 10000 / 15; Gencore Phoenix 12000 / 17 |
| Membrane Ray | balanced fighter | 13500 | 18 | Manta-shaped membrane wing with a carapace spine and one green eye. | Gencore Phoenix 12000 / 17; Gencore Maelstrom 15000 / 20 |
| Leech Lancer | assault | 18500 | 23 | Long leech body behind a bone lance, hooked tendril arms, flesh joints. | Gencore II 17000 / 22; MicroCorp Stalker 20000 / 25 |
| Carapace Beetle | heavy armored | 26000 | 28 | Thick split shell with a bone horn and a glowing slit between the plates. | MicroCorp Stalker-B 25000 / 27; Prototype Stalker-C 50000 / 28 |
| Hive Mother | flagship | 55000 | 29 | Bulbous hive body with membrane wings and four neon egg sacs. | Prototype Stalker-C 50000 / 28; SuperCarrot 65000 / 26 |

Prices and armor stay inside the range of the original ships at the same price
(armor never more than one point above the closest original). Speed is the same
for every ship: the engine has no per-ship speed (MODDING.md, removed fields),
so a role is told by armor and price only.

## Limits

- **Ship slots**: mods share 14 ship slots (MODDING.md §5.1). The Alien Ship
  Pack takes 8 and this pack 6: together they fill every slot, so a third mod
  that adds ships gets a "no free slot" error while both are enabled.
- **Ship images**: each PNG takes one place of the mods' ship image table,
  which holds five poses for each of the 14 slots (MODDING.md §5.5, ADR 0134).
  This pack's 30 PNGs and the Alien Ship Pack's 40 fit together.
- **Shop**: the six ships are offered with the level's own ships and the Alien
  Ship Pack's; the ship list scrolls (MODDING.md §5.9).

## How it was made

Drafts by PixelLab (`create_image_pixflux`, 48x56, top-down, forced palette),
then `utils/bio_ships_art.py`, which reuses `utils/mod_art_clean.py`: fit into a
per-ship box (the scout is drawn smaller), palette snap, mirrored hull, upper-left
shading, hand-placed glow on the Spore Wisp's sac and the Beetle's slit, rim,
then the four banking poses.

```
A="python utils/bio_ships_art.py ship"; O=rewrite/mods/bio_ships_pack/sprites/ships
$A wisp.png   $O bio_spore_wisp      --box 18x24 --mirror --glow-box 9,10,14,16
$A thorn.png  $O bio_thornwing       --box 22x26 --mirror
$A ray.png    $O bio_membrane_ray    --box 22x26 --mirror
$A leech.png  $O bio_leech_lancer    --box 22x26 --mirror
$A beetle.png $O bio_carapace_beetle --box 22x26 --vflip --mirror --glow-box 11,12,12,19
$A hive.png   $O bio_hive_mother     --box 22x26 --mirror
```
