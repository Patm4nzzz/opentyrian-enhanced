# Asset rights

The code of this repository is under GPL v2 (`COPYING`). The game data and a
few third-party assets are not covered by that licence. None of the data files
below is copied into `rewrite/`: the Godot client reads them from `data/` or
converts them locally (output ignored by git). The packages carry a short
version of this page in `DATA-NOTICES.txt` (written by
`rewrite/scripts/package.sh`).

| Item | Status |
|---|---|
| Tyrian 2000 data (`*.lvl`, `levels*.dat`, `*.hdt`, `*.shp`, `shapes*.dat`, `palette.dat`, `music.mus`, `*.snd`, `*.pic`…) | Freeware since 2004, with no written licence in the repository; not covered by the code's GPL. |
| `data/soundfont.sf2` (TimGM6mb, Tim Brechbill) | GPL v2. Licence and copyright notice in `rewrite/licenses/TimGM6mb-LICENSE.txt`, copied into the packages' `licenses/` folder. Played for the MIDI music (ADR 0130). |
| `data/midi/*.mid` (41) | Third-party transcriptions (karlsitretro.com) of the music by Alexander Brandon and Andreas Molnar; no licence included, rights not verified. Played at the player's choice (ADR 0130) and copied into the packages, with the mention in `DATA-NOTICES.txt`. |
| HD (AI) images (`upscaled/`) | Derived from the freeware data by the 4x-PixelPerfectV4 model (WTFPL), run with spandrel (MIT) and PyTorch (BSD). |
| rustysynth (MIDI synthesiser, linked into the game) | MIT; text in `rewrite/licenses/rustysynth-LICENSE.txt`, copied into the packages. midly: Unlicense. |
| xBRZ freescale shader | libretro/glsl-shaders; Hyllian (MIT), Zenju (GPL v3); notices kept in the shader source. |

All new code is under GPL v2, like the repository.
