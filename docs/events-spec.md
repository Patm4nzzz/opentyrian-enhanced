# Level event specification (OpenTyrian2000 Enhanced)

Document extracted **from the C code only** (read-only). All citations
refer to `src/<file>:<function>:<line>`. Anything that could not be
confirmed in the source is marked **undetermined**.

Dispatcher: `src/level_events.c:JE_eventSystem` (l. 171-1287), a single
`switch (eventRec[eventLoc-1].eventtype)`.

---

## 1. The `JE_EventRecType` structure

Declared in `src/varz.h:137-144`, global array
`struct JE_EventRecType eventRec[EVENT_MAXIMUM + 1]` in `src/tyrian2.c:122`
(`EVENT_MAXIMUM = 2500`, `src/lvlmast.h:24`; the +1 is the sentinel).

| Field       | C type     | Read order in the .lvl        | Usual use |
|-------------|------------|-------------------------------|----------------|
| `eventtime` | `uint16_t` | 1 (u16)                       | Scroll position at which the event fires |
| `eventtype` | `uint8_t`  | 2 (u8)                        | Type number (switch case) |
| `eventdat`  | `int16_t`  | 3 (s16)                       | Main parameter (enemy no., speed, jump target…) |
| `eventdat2` | `int16_t`  | 4 (s16)                       | Second parameter (often spawn X position) |
| `eventdat3` | `int8_t`   | 5 (s8)                        | Often Y speed (`eyc`) / selector |
| `eventdat5` | `int8_t`   | 6 (s8)                        | Often Y offset / animation cycle |
| `eventdat6` | `int8_t`   | 7 (s8)                        | Often `fixedmovey` / background |
| `eventdat4` | `uint8_t`  | 8 (u8)                        | Almost always a `linknum` (enemy group) |

Note: on disk the order is `dat3, dat5, dat6, dat4`
(`src/level_load.c:985-995`). Record = 11 bytes on disk
(2 + 1 + 2 + 2 + 1 + 1 + 1 + 1; see `formats-spec.md` § 1.2 and
`rewrite/crates/tyrian-formats/src/lvl.rs`, `Event::read`).

Loading (`src/level_load.c:982-1005`): `maxEvent` (u16) is read then clamped to
`EVENT_MAXIMUM`; after the last event, slot `eventRec[maxEvent]` is
zeroed with `eventtime = 65500` (sentinel).

Recurring "magic" values in the fields:
- `-99` in `eventdat`/`eventdat2`/`eventdat6`: "do not modify this field" (events 19, 20, 27, 55, 74); for a spawn, `eventdat2 == -99` = do not set X/Y (`level_helpers.c:125`).
- `-200` in the `eventdat2` of a spawn: random X 24..231 (`level_helpers.c:119-123`, written into the record — so the draw is frozen for later replays).
- `eventdat3` in 80..89: replace `eventdat4` with `newPL[eventdat3-80]` (link chosen by event 75), events 19, 20, 27, 55.

## 2. Scheduling

- Initialization at level start: `eventLoc = 1; curLoc = 0;` (`tyrian2.c:496-497`). `eventLoc` is a **1-based** index (the current event is `eventRec[eventLoc-1]`).
- On each frame of the main loop (`tyrian2.c:1009-1010`):
  ```c
  while (eventRec[eventLoc-1].eventtime <= curLoc && eventLoc <= maxEvent)
      JE_eventSystem();
  ```
  All events with `eventtime <= curLoc` are executed in the same frame, in file order. `JE_eventSystem` always ends with `eventLoc++` (`level_events.c:1286`). Events must therefore be sorted by increasing `eventtime` (not checked at load time — whether the engine tolerates disorder other than by late execution is undetermined).
- Advancing `curLoc`: `curLoc += backMove` each time the `map1YDelay` counter drops back to 0 (`tyrian2.c:1039-1043`), i.e. `curLoc` follows the scrolling of background 1 (`backMove` pixels per step, one step every `map1YDelayMax` frames). If `forceEvents && !backMove`, `curLoc++` on every frame (`tyrian2.c:1026-1027`): the script keeps advancing even with the map stopped.
- An "endless" mode may override the waves right afterwards (`endless_on_frame()`, `tyrian2.c:1013`) — out of scope.
- Jumps: `JE_eventJump(jump)` (`level_helpers.c:186-205`): if `jump == 65535`, `curLoc = returnLoc`; otherwise `returnLoc = curLoc + 1; curLoc = jump;` then `eventLoc` is moved back to the first event with `eventtime >= curLoc` (minus one, compensated by the final `eventLoc++`). `event_jump_relative(delta)` (`level_events.c:150-169`): `eventLoc += delta`, clamped to `1..maxEvent`, used by 61, 63, 66, 75, 80.

## 3. Enemy creation path

`JE_createNewEventEnemy(enemyTypeOfs, enemyOffset, uniqueShapeTableI)`
(`src/level_helpers.c:96-184`):
1. Looks for a free slot (`enemyAvail[i] == 1`) in the window `[enemyOffset, enemyOffset+25)`. Windows: `0` = sky, `25` = ground, `50` = "top" (background 3), `75` = ground 2. The slot found (1-based) is stored in the global `b` (0 = none).
2. `JE_makeEnemy(&enemy[b-1], eventdat + enemyTypeOfs, uniqueShapeTableI)` (`src/enemy_spawn.c:46`) initializes the enemy from `enemyDat[eDatI]` (sprite bank = `uniqueShapeTableI` if > 0, otherwise `enemyDat[].shapebank`). May return 1 (slot left free, "No Powerups" mutator).
3. Position: if `eventdat2 != -99`, `ey = -28` and `ex = eventdat2 - (mapX-1)*24` (sky, `ey -= backMove2`), an extra `-12` for ground/ground2 (`ey -= backMove`), specific `mapX3` formula for window 50 (`ey -= backMove3`, `background3x1`/`background3x1b` adjustments).
4. If `smallEnemyAdjust` and `size == 0`: `ex -= 10; ey -= 7`.
5. Always: `ey += eventdat5; eyc += eventdat3; linknum = eventdat4; fixedmovey = eventdat6`.
6. Mutators: mirror (`ex = 264 - ex`, `exc = -exc`), fast enemies (×1.5 on `exc`/`eyc`).

`JE_newEnemy(enemyOffset, eDatI, uniqueShapeTableI)` (`src/enemy_spawn.c:23-44`): a variant that does not read the event (no positioning), used by event 59/68.

## 4. Event type table

Notation: `d1` = `eventdat`, `d2..d6` = `eventdat2..6`. All lines are
in `src/level_events.c:JE_eventSystem` unless stated otherwise.

| Type | Name (descriptive) | Parameters | Exact effect | Source |
|---|---|---|---|---|
| 0 | Header record | — | No effect (explicit no-op). | l.1245 |
| 1 | Starfield speed | d1 | `starfield_speed = d1`. (Comment: 0 occurrences in the shipped .lvl files.) | l.178 |
| 2 | Background scroll speed | d1, d2, d3 | `map1YDelay=map1YDelayMax=map2YDelay=map2YDelayMax=1`; `backMove=d1`, `backMove2=d2`, `backMove3=d3`; `explodeMove = d2>0 ? d2 : d1`; if `backMove>0` then `stopBackgroundNum=0`. | l.186 |
| 3 | Predefined slow scroll | — | `backMove=1, map1YDelay(Max)=3, backMove2=1, map2YDelay(Max)=2, backMove3=1`. | l.206 |
| 4 | Map stop | d1 | `stopBackgrounds=true`; `stopBackgroundNum = 1` if d1∈{0,1}, `2` if d1=2, `3` if d1=3 (other values: unchanged). Automatic resume when no enemy remains on screen (`tyrian2.c:1736-1742`: `backMove=1,2,3`, `explodeMove=2`). | l.216 |
| 5 | Loading of enemy sprite banks | d1..d4 | For each bank i∈0..3 (value `max(di,0)`) that differs from `enemySpriteSheetIds[i]`: loads `shapeFile[di-1]` (+ override, + HD sheet in upscaled mode) or frees it if 0. Value > number of `shapeFile` entries is ignored with a log. | l.234 |
| 6 | Ground enemy | standard spawn | `JE_createNewEventEnemy(0, 25, 0)`. | l.294 |
| 7 | "Top" enemy (background 3) | standard spawn | `JE_createNewEventEnemy(0, 50, 0)`. | l.298 |
| 8 | Stars OFF | — | `starActive = false`. | l.302 |
| 9 | Stars ON | — | `starActive = true`. | l.306 |
| 10 | Ground enemy 2 | standard spawn | `JE_createNewEventEnemy(0, 75, 0)`. | l.310 |
| 11 | End of level | d1 | If `allPlayersGone` or `d1==1`: `reallyEndLevel=true` (immediate end). Otherwise if `!endLevel`: `readyToEndLevel=false; endLevel=true; levelEnd=40`. | l.314 |
| 12 | Custom 4×4 enemy | std spawn, d6 = window | Window by d6: 0/1→25, 2→0, 3→50, 4→75; **d6 reset to 0 in the record**; creates 4 enemies `eventdat+0..3`: (+0,0), (+24,0), (0,−28), (+24,−28). | l.327 |
| 13 | Random spawn OFF | — | `enemiesActive = false`. | l.362 |
| 14 | Random spawn ON | — | `enemiesActive = true` (random spawn from `levelEnemy[]`, `tyrian2.c:1145`). | l.366 |
| 15 | Sky enemy | standard spawn | `JE_createNewEventEnemy(0, 0, 0)`. | l.370 |
| 16 | Text window + voice | d1 ∈ 1..9 | `JE_drawTextWindow(outputs[d1-1])` and `soundQueue[3] = windowTextSamples[d1-1]`, subject to voice deduplication (`voice_dedup_allow`). Enhanced fix: if `9 < d1 <= ENEMY_NUM`, replayed as a type 10 (ground 2 spawn) with a log; otherwise ignored. | l.374 |
| 17 | Ground enemy at screen bottom | std spawn, d5 | As 6 then `ey = 190 + d5`. | l.436 |
| 18 | Sky enemy at screen bottom | std spawn, d5 | As 15 then `ey = 190 + d5`. | l.442 |
| 19 | Global enemy movement | d1=exc, d2=eyc, d3=selector, d4=link, d5=cycle, d6=fixedmovey | Range: d3=0 → slots 0..99 filtered by `linknum==d4`; 1→25..49, 2→0..24, 3→50..74, 99→0..99 **without filter**; 80..89 → `d4=newPL[d3-80]` then 0..99 filtered. For each enemy: `exc=d1` if ≠−99; `eyc=d2` if ≠−99; `fixedmovey=d6` if ≠0 (−99 → 0); `enemycycle=d5` if >0. Any other d3 value: empty range (no effect). | l.448 |
| 20 | Global acceleration | d1, d2, d3, d4, d5, d6 | d3∈80..89 → `d4=newPL[...]`. For each active enemy (`enemyAvail!=1`) with `linknum==d4` or d4==0: if d1≠−99: `excc=d1, exccw=exccwmax=|d1|, exccadd=±1`; same for Y with d2 (`eycc…`); `enemycycle=d5` if d5>0; if d6>0: `ani=d6, animin=d5, animax=0, aniactive=1`. | l.515 |
| 21 | Background 3 on top | — | `background3over = 1`. | l.561 |
| 22 | Background 3 below | — | `background3over = 0`. | l.565 |
| 23 | "Top" enemy at bottom | std spawn, d5 | As 7 then `ey = 180 + d5`. | l.569 |
| 24 | Global animation | d1=ani, d2=cycle, d3=mode, d4=link | Enemies with `linknum==d4`: `aniactive=1, aniwhenfire=0`; d2>0 → `enemycycle=animin=d2` otherwise `enemycycle=0`; d1>0 → `ani=d1`; d3=1 → `animax=ani`; d3=2 → `aniactive=2, animax=ani, aniwhenfire=2`. | l.575 |
| 25 | Global armor change | d1, d4 | Enemies with `linknum==d4` (or all if d4==0): `armorleft=d1`; in `galagaMode`: `roundf(d1 * (difficultyLevel/2))`. | l.609 |
| 26 | Small-enemy adjustment | d1 | `smallEnemyAdjust = d1` (−10/−7 offset on size-0 spawns). | l.622 |
| 27 | Global acceleration reversal | d1, d2, d3, d4 | d3∈80..89 → `d4=newPL[...]`. Enemies with link d4 (or all): `exrev=d1`, `eyrev=d2` (if ≠−99); `filter=d3` if 0<d3<17. | l.626 |
| 28 | "Top" enemies below the rest | — | `topEnemyOver = false`. | l.644 |
| 29 | "Top" enemies above | — | `topEnemyOver = true`. | l.648 |
| 30 | Background speed (variant) | d1, d2, d3 | As 2 but `explodeMove = backMove2` unconditionally and does not touch `stopBackgroundNum`. | l.652 |
| 31 | Forcing the fire rate | d1..d3, d4, d5 | Enemies with link d4 (99 = all): `freq[0..2] = d1,d2,d3`, `eshotwait[0..2]=1`; if `launchtype>0`: `launchfreq=d5, launchwait=1`. | l.664 |
| 32 | "Top" enemy at y=190 | std spawn | `JE_createNewEventEnemy(0,50,0)` then `ey = 190`. | l.685 |
| 33 | Object dropped on death ("enemy from enemy") | d1=enemy no., d4=link | Ignored if d1∈{512,513} and (2 players / onePlayerAction / superTyrian). Super Arcade: 534→827. Outside superTyrian: 533 → 829+rand(6) if `lives==11` or `rand%15 < lives`. superTyrian: 534 → 828+{1,2,4,5}[rand%4]. Then enemies with `linknum==d4`: `enemydie = d1`. **The record is modified** (d1 rewritten). | l.691 |
| 34 | Music fade | — | If `firstGameOver`: `musicFade=true; tempVolume=tyrMusicVolume`. | l.720 |
| 35 | New music | d1 | If `firstGameOver`: `play_song(d1-1)` + `set_volume`. Always `musicFade=false`. | l.728 |
| 36 | Ready to end the level | — | `readyToEndLevel = true`: the end is triggered when no enemy remains on screen or exploding (`tyrian2.c:1720-1735` → `endLevel=true, levelEnd=40`). | l.737 |
| 37 | Random enemy frequency | d1 | `levelEnemyFrequency = d1` (default 96; spawn if `rand%100 > freq`, `tyrian2.c:1145`). | l.741 |
| 38 | Absolute position jump | d1 | `curLoc = d1` then `eventLoc` = (0-based) index of the last event with `eventtime <= curLoc` (the final `eventLoc++` makes it 1-based → re-executes… see note 5.1). | l.745 |
| 39 | Global link change | d1, d2 | Enemies with `linknum==d1` → `linknum=d2`. | l.756 |
| 40 | Continuous enemy damage | — | `enemyContinualDamage = true` (used in `collision.c:143`, damage propagated to link `temp-100`). | l.764 |
| 41 | Enemy purge | d1 | d1==0: all slots freed (`enemyAvail[]=1`); otherwise only slots 0..24 (sky). | l.768 |
| 42 | Background 3 mode 2 | — | `background3over = 2`. | l.780 |
| 43 | Background 2 on top | d1 | `background2over = d1`. | l.784 |
| 44 | Color / brightness filter | d1..d6 | `filterActive = d1>0`, `filterFade = d1==2`, `levelFilter=d2`, `levelBrightness=d3`, `levelFilterNew=d4`, `levelBrightnessChg=d5`, `filterFadeStart = d6==0` (animated in `backgrnd.c:281-291`). | l.788 |
| 45 | Dropped object (arcade / 2 players only) | d1, d4 | Outside superTyrian: same 533→829+rand(6) rule; then, **only if** `twoPlayerMode || onePlayerAction`, `enemydie=d1` for link d4. | l.798 |
| 46 | Difficulty change | d1, d2, d3 | d3≠0 → `damageRate=d3`. If d2==0 or 2 players/onePlayerAction: `difficultyLevel += d1`, clamped to `DIFFICULTY_EASY..DIFFICULTY_10`. | l.818 |
| 47 | Global armor (variant) | d1, d4 | Enemies with link d4 (or all): `armorleft = d1` (no galaga scaling). | l.832 |
| 48 | Opaque background 2 | — | `background2notTransparent = true`. | l.840 |
| 49-52 | Custom enemy | d1=graphic, d3=bank, d6=armor, + std spawn | Writes `enemyDat[0].armor=d6`, `enemyDat[0].egraphic[0]=d1`; temporarily sets d1/d3/d6 to 0 and spawns `enemyDat[0]` with `uniqueShapeTableI=d3` in window 49→25, 50→0, 51→50, 52→75; then restores d1/d3/d6. (d3 is therefore not added to `eyc`.) | l.844 |
| 53 | Forced events | d1 | `forceEvents = (d1 != 99)` (see §2: `curLoc++` with the map stopped). | l.879 |
| 54 | Jump (with return) | d1 | `JE_eventJump(d1)` (65535 = return to `returnLoc`). | l.883 |
| 55 | Acceleration (xaccel/yaccel) | d1, d2, d3, d4 | d3∈80..89 → `d4=newPL[...]`. Link d4 (or all): `xaccel=d1`, `yaccel=d2` (if ≠−99). | l.887 |
| 56 | Ground enemy 2 at y=190 | std spawn | `JE_createNewEventEnemy(0,75,0)` then `ey = 190`. | l.903 |
| 57 | Jump on death of enemy 254 | d1 | `superEnemy254Jump = d1`; when an enemy with link 254 dies and the value is > 0: `JE_eventJump(superEnemy254Jump)` (`collision.c:224-225`). | l.909 |
| 58 | Launch type | d1, d4 | Link d4 (99 = all): `launchtype = d1`. Source comment: implementation taken from ArcTyr, fidelity to T2000 not guaranteed. | l.913 |
| 59 / 68 | Enemy replacement | d1=enemy no., d4=link | For each slot (0..ENEMY_MAX) with link d4 (or all if 0): `JE_newEnemy(slot's window, d1, 0)` at the same `ex/ey` position, then the old slot is freed. Also walks free slots (no `enemyAvail` test). Same ArcTyr remark. | l.922 |
| 60 | Special enemy (flag) | d1, d2, d4 | Link d4: `special=true`, `flagnum=d1`, `setto = (d2==1)` (sets `globalFlags` on its death — death logic **not verified here**). | l.947 |
| 61 | Conditional jump on flag | d1=flag no. 1..10, d2=value, d3=delta | If `globalFlags[d1-1] == d2`: `event_jump_relative(d3)`. d1 out of bounds → ignored + log. | l.959 |
| 62 | Sound effect | d1 ∈ 1..SOUND_COUNT(40) | `soundQueue[3] = d1` (voice gate / 3 s limit). | l.975 |
| 63 | Jump if not 2 players | d1=delta | If `!twoPlayerMode && !onePlayerAction`: `event_jump_relative(d1)`. (0 shipped occurrences.) | l.998 |
| 64 | Smoothie (visual effect) | d1=no. 1..9, d2, d3 | Ignored if d1==6 in 2 players and difficulty > Normal. `smoothies[d1-1] = d2`; `smoothie_data[(d1==5?3:d1)-1] = d3`. | l.1007 |
| 65 | Background 3 at 1:1 | d1 | `background3x1 = (d1 == 0)`. | l.1027 |
| 66 | Jump by difficulty | d1, d2=delta | If `initialDifficulty <= d1`: `event_jump_relative(d2)`. | l.1031 |
| 67 | Level timer | d1, d2, d3 | `levelTimer = (d1==1)`, `levelTimerCountdown = d3*100`, `levelTimerJumpTo = d2`. At 0: `JE_eventJump(levelTimerJumpTo)` (`tyrian2.c:1406-1410`). | l.1036 |
| 69 | Invulnerability | d1 | `invulnerable_ticks = d1` for each player. | l.1042 |
| 70 | Jump if groups eliminated | d1=target, d2..d4=links | d2==0: if no living enemy with link 1..19 → `JE_eventJump(d1)`. Otherwise: if none alive with link d2, nor d3 (if ≠0), nor d4 (if ≠0) → `JE_eventJump(d1)`. | l.1047 |
| 71 | Jump by map position | d1=target, d2=threshold | If `((mapYPos - &megaData1.mainmap)/sizeof(uint8_t*))*2 <= d2` → `JE_eventJump(d1)`. | l.1066 |
| 72 | Background 3 variant b | d1 | `background3x1b = (d1 == 1)`. | l.1071 |
| 73 | Sky enemies above everything | d1 | `skyEnemyOverAll = (d1 == 1)`. | l.1075 |
| 74 | Bounce parameters | d1, d2, d4, d5, d6 | Link d4 (or all): `xminbounce=d5`, `yminbounce=d6`, `xmaxbounce=d1`, `ymaxbounce=d2` (each if ≠−99). | l.1079 |
| 75 | Random choice of a link | d1..d2=link range, d3=newPL slot (80..89), d4=jump | See note 5.3. | l.1099 |
| 76 | Return when screen is empty | — | `returnActive = true`; when `enemyOnScreen == 0`: `JE_eventJump(65535)` (return to `returnLoc`) and `returnActive=false` (`tyrian2.c:1357-1361`). | l.1155 |
| 77 | Map repositioning | d1, d2 | `mapYPos = &megaData1.mainmap[0][0] + d1/2`; `mapY2Pos = &megaData2.mainmap[0][0] + (d2>0 ? d2 : d1)/2`. | l.1159 |
| 78 | Galaga fire rate | — | `galagaShotFreq++` if < 10. | l.1174 |
| 79 | Boss health bar(s) | d1, d2 | `boss_bar[0].link_num = d1`, `boss_bar[1].link_num = d2`; emits `EVENT_BOSS_SPAWNED` (link d1) on the event bus. (No music launch here: boss music goes through event 35.) | l.1179 |
| 80 | Jump if 2 players | d1=delta | If `twoPlayerMode`: `event_jump_relative(d1)`. | l.1188 |
| 81 | Background 2 wrap (WRAP2) | d1, d2 | `BKwrap2 = megaData2.mainmap + d1/2`, `BKwrap2to = megaData2.mainmap + d2/2`. | l.1193 |
| 82 | Give a special weapon | d1 ∈ 0..SPECIAL_NUM | Player 1 only: `items.special = d1`, reset of the SPECIAL and SPECIAL2 `shotMultiPos`/`shotRepeat`. (0 shipped occurrences.) | l.1200 |
| 83 | Map stop (T2000) | d1 | Identical to 4 (same case). | l.217 |
| 84 | Timer (Timed Battle) | d1, d2, d3 | Only if `timedBattleMode`; otherwise a copy of 67. | l.1219 |
| 85 | Dropped object (Timed Battle) | d1, d4 | If `timedBattleMode`: `enemydie = d1` for link d4. | l.1229 |
| 99 | Random explosions | d1 | `randomExplosions = (d1 == 1)`; each frame, 1 chance in 10 of a big random explosion (`tyrian2.c:1352-1353`). | l.1241 |
| other | Unknown type | — | Ignored, logged with the full payload (known example: type 108 in tyrian4.lvl, EYESPY). | l.1255 |

"standard spawn" = d1 enemy no. (`enemyDat`), d2 X (−99 = no position, −200 = random), d3 added to `eyc`, d4 `linknum`, d5 added to `ey`, d6 `fixedmovey` (see §3).

Types 86..98 and ≥ 100 have **no** case: they fall into `default`.

## 5. Notes on complex events

### 5.1 Event 38 (absolute jump)
`new_event_loc` receives `tempW` (0-based) of the **last** record such that
`eventtime <= curLoc`, then `eventLoc = new_event_loc` and the final `eventLoc++` ⇒
`eventLoc` designates that record in 1-based terms, so it will be replayed on
the current frame if it is still `<= curLoc`. If several events share
that `eventtime`, only the last one and those after it are replayed. Unlike
54, `returnLoc` is not modified. The exact behavior on loops
(38 replaying itself if `d1 >= its eventtime`) has not been tested — **undetermined**
in practice.

### 5.2 Links (`linknum`, `eventdat4`)
Almost all "global" events target enemies by `linknum`
(set at creation = d4). Conventions found: d4 == 0 = all (20, 25, 27,
47, 55, 59, 74); d4 == 99 = all (31, 58); 19 uses d3 as a range
selector; 24, 33, 39, 45, 60, 85 have **no** wildcard (0 only targets link 0).
Link 254: trigger for 57. Links > 40: damage sharing in slices of 20
(`collision.c:144`).

### 5.3 Event 75 (random pair)
1. Looks for a living enemy (`enemyAvail==0`), vertically motionless (`eyc==0`), with a link in `[d1, d2]`.
2. If one exists: draws a random `temp` in `[d1, d2]` until such an enemy is found (potentially long loop but terminating since at least one candidate exists); if d3 ∈ 80..89: `newPL[d3-80] = temp`.
3. Otherwise: `newPL[d3-80] = 255` (if d3 ∈ 80..89); if d4 > 0: `curLoc = eventRec[eventLoc-1+d4].eventtime - 1` then `event_jump_relative(d4-1)` (skips d4 events).
Events 19/20/27/55 then read `newPL` via d3 ∈ 80..89.

### 5.4 End of level
Two paths: 11 (immediate, `levelEnd=40` or `reallyEndLevel`) and 36
(deferred until the screen is empty, `tyrian2.c:1720-1735`). The transition to the next
level then depends on the `.lvl`/section script (`level_load.c`, e.g. `]t[`
with `levelTimerCountdown == 0`, l.333-339) — out of scope for this document.

### 5.5 Record mutations
Some events **rewrite** `eventRec[]` and therefore change their effect
if replayed after a jump: 12 (d6 → 0), 19/20/27/55 (d4 ← newPL),
33/45 (d1 rewritten by the object draw), spawn with d2 = −200 (X frozen at the
first draw). 49-52 restore their fields.

### 5.6 Additions specific to the Enhanced version
- Safety bounds with `log_warn` on 5, 16, 61, 62, 64, 82 and on all relative jumps.
- 16: reinterpretation as a spawn (type 10) of two corrupted records in the shipped levels.
- Voice deduplication (16, 62) by event position + 3 s limit per sample, reset at level load (`level_events_reset_voice_dedup`, `tyrian2.c:498`).
- 79 emits `EVENT_BOSS_SPAWNED` on `event_bus`.

## 6. Summary

- **87 explicitly handled types**: 0 to 85 (all present) + 99; 4≡83 and 59≡68 share their code; 49-52 share a single block. Any other number → `default` (ignored + log).
- Undetermined: fine semantics of the visual fields (`smoothies`, `filter`, `background3over=2`), behavior of `JE_makeEnemy` beyond the initialization described, and death logic of `special` enemies (event 60) — not read in detail.
