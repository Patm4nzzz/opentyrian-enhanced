# Saves of the C version (test fixtures)

Read-only copies, taken on 2026-10-04 in WSL (the originals were not modified):

| File | Origin | Content |
|---|---|---|
| `maintainer-opentyrian.dat` | `~/.config/opentyrian2000/opentyrian.dat` (June 12, 2026) | the maintainer's save |
| `root-golden-opentyrian.dat` | `/root/.config/opentyrian2000/opentyrian.dat` | state written by the golden master sessions |
| `install-opentyrian.dat` | `/opentyrian2000/opentyrian.dat` | installation copy |

Format: `tyrian-formats/src/save.rs` (`OT2K` container, `GAME` section = former `opentyrian.sav`).
Test: `cargo test -p tyrian-formats --test saves`.

Games in progress (slot 11 "LAST LEVEL" written by the C version at the start of each level), copied from
`<repo>/tmp/tests/sessions/<date>/<harnessNNN>/state/opentyrian2000/opentyrian.dat`:

| File | Slot 11 |
|---|---|
| `harness151.dat` | episode 1, section 40 "ASSASSIN", 1,128,289 credits, slot named "ULTIMO LIVELLO" |
| `harness181.dat` | episode 2, section 11 "BOTANY B", 308,402 credits (+ slot 1 "RSI1") |
| `harness321.dat` | episode 5, section 17 "STATION" |
| `harness365.dat` | episode 4, section 35 "CORE", 110,917 credits |

Full sweep (`save_dump` over the 162 `opentyrian.dat` files of the WSL disk): 161 identical round trips
(container and GAME section); the 162nd (`/tmp/.../.config/opentyrian/opentyrian.dat`) has no GAME section.
