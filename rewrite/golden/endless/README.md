# Endless mode golden masters

Produced by `c-final:utils/golden/record_endless.sh` (C from `master`, `make GOLDEN=1`):
`--goto=endless` with `GOLDEN_ENDLESS_OUT` calls `endless.c:endless_golden_dump`
right after the `rng_seed` of `endless_start`, with the seed `GOLDEN_SEED`, then quits.

Text format, one line per fact:

- `endless seed S picks N`: header;
- `pool EP N s1 … sN`: pool from `build_level_pool` for episode EP;
- `pick K EP SECTION`: K-th result of `pick_next_level` after
  `build_rotation` + `fill_block_queue` (like `endless_start`);
- `mult L BITS a1 … a254`: `endless_armor_mult_for_levels(L)` (IEEE 754 bits in
  hexadecimal) then the armor scaled by `enemy_spawn.c` for a
  base armor of 1..254;
- `end`.

Checked by `cargo test -p tyrian-sim --test endless_golden` (module
`tyrian-sim/src/endless.rs`).
