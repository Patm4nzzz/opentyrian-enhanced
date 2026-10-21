#!/usr/bin/env bash
# Counts special weapon triggers (fire_button_held going from false to
# true = one call to JE_specialComplete by the shot) in each golden master
# e*-special-*. Usage (WSL, from rewrite/): bash scripts/count_specials.sh
set -euo pipefail
bin=${CARGO_TARGET_DIR:-target}/release/examples/special_trace
for f in golden/e*-special-*.gm; do
  n=$(($(wc -l <"$f") - 5))
  "$bin" "$f" "$n" 2>/dev/null | awk -v f="$f" '
    { held = ($0 ~ /held true/) ; if (held && !prev) c++ ; prev = held
      if ($0 ~ /flare [1-9]/) fl++ }
    END { printf "%s triggers=%d flare_ticks=%d\n", f, c, fl }'
done
