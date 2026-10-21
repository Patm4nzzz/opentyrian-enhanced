#!/usr/bin/env bash
# UI regression: replays
# all the screen sequences of the visual and interface parity tables,
# then compares each Godot image sequence rank by rank
# (visual_diff.py seqdiff) to the C reference and, if it exists, to a
# Godot reference taken before a refactoring.
#
# Usage (WSL, from anywhere):
#   ui_regression.sh godot-ref REF_DIR      Godot capture of the "godot" sequences (screens
#       deliberately different from the C) and writes REF_DIR/<sequence>.sha1.
#   ui_regression.sh godot OUT_DIR [REF_C_DIR] [REF_GODOT_DIR] [sequence...]
#       Godot capture (in parallel) then table
#       "sequence / C images / Godot images / ranks != C / ranks != Godot before".
#       REF_C_DIR contains the <sequence>.sha1 files (one fingerprint per image):
#       frozen references rewrite/golden/ui, captured long ago on the C version
#       (tag c-final) which is no longer in the repository. They are no longer regenerated.
# Variables: JOBS (simultaneous Godot captures, default 8).
# Return code: 0 if all the sequences have 0 differing ranks and if no
# Godot capture failed (non-zero code, ERROR or SCRIPT ERROR in its log).
set -uo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# name | C keys (xdotool) | Godot scene | options (FIXED_SEED, shop, godot, day)
# "day" option: UTC noon frozen on the Godot side (--unix-time), otherwise "Night Player"
# (achievements.c, local time < 5 am) unlocks when CI runs at night and
# the achievements screen differs from the C reference, captured in daytime.
# "godot" option: screen deliberately different from the C, the reference is a
# validated Godot capture (fingerprints written by "ui_regression.sh godot-ref").
# This is the case of the Setup submenus, reduced to the wired options
# (docs/setup-options.md); Key Bindings opens the Controls screen (Node2D, outside the
# 320x200 framebuffer: covered by controls_test.gd); Joystick, the Gamepad screen
# (joystick_settings_test.gd).
# Instructions: the "Keys and mouse" topic is added to the menu (divergence no. 57),
# Godot reference.
# "godot:<options>" option: Godot reference too, with capture options in
# addition (pause menu and in-game help, placed on the ep1_09 level image at
# tick 1: the C cannot freeze a game at the same tick AND open its menu,
# GOLDEN_FREEZE_AT blocks the event loop).
SEQUENCES=(
  "title|Down Down|menu|"
  "quit-box|Escape Left Right Escape Up Return Tab Escape|menu|"
  "high-scores|Down Down Return Right Right Right Right Right Right Right Left Left a Escape|menu|seed"
  "instructions|Down Down Down Return Down Up Up Down Return Right Right Right Left Escape Down Down Return Right Right Return Left Left Left Left Down Return Right Right Right Right Down Down Down Return|menu|godot"
  "load|Down Return Down Down Up Right Down Return Left Right Right Up Return|menu|"
  "achievements|Down Down Down Down Down Return Down Down Down Down Down Down Down Down Down Down Up Right Down Right Right Right Right Right Left Left Down Escape|menu|day"
  "replays|Up Up Up Return Down Up Return Escape|menu|"
  "shop-1|DDE DDD E X X|shop|shop:Down,Down,Enter,Down,Down,Down,Enter,Escape"
  "shop-2|DDE E X DDDE X X|shop|shop:Down,Down,Enter,Enter,Escape,Down,Down,Down,Enter,Escape,Escape"
  "setup|Down Down Down Down Return Down Down Up Escape|menu|godot"
  "graphics|Down Down Down Down Return Return Return Down Up Escape Down Escape Escape|menu|godot"
  "sound|Down Down Down Down Return Down Return Left Left Return Return Down Right Right Down Right Left Return Down Return Down Return Escape|menu|godot"
  "gameplay|Down Down Down Down Return Down Down Return Return Down Return Down Return Down Right Left Down Return Escape|menu|godot"
  # New game (title_screen.c:newGame, menus.c): choice of mode,
  # episode, difficulty and timed battle; Escape goes back
  # one screen; Shift+G, Shift+] then "LORD" show the three hidden difficulties.
  "new-game|Return Down Up Return Down Down Escape|menu|"
  "difficulty|Return Return Return Down Down Escape Escape|menu|"
  "timed-battle|Return Down Down Down Return Down Return Down Escape Escape|menu|"
  "lord|Return Return Return shift+g shift+bracketright l o r d Down Down Down Down Down Escape Escape|menu|"
  "pause-menu|Down Right Right Left Return Down Right Left Down Right Right Right Right Right Left Down Down Return Left Right Escape Up Escape|level|godot:--pause-menu --unix-time=43200"
  "in-game-help|space|level|godot:--in-game-help --unix-time=43200"
  "destruct||destruct|destruct"
  # Mouse only (--mouse=x:y:b): hover on the title, click on Setup, hover and
  # click on Sound, one-row wheel, two right clicks (Sound then Setup),
  # right click on the title (Quit box). Godot reference (godot option).
  "mouse||menu|godot|160:100:-,160:140:l,160:40:-,160:51:l,160:40:-,160:40:d,160:55:r,160:55:r,160:180:r"
)
# Leading ranks ignored for the shop: level played (C), title (Godot);
# the C sequence continues after the shop (next level): cut to the
# Godot length (C 255-268 = Godot 142-155).
SHOP_SKIP_C=255
SHOP_SKIP_G=142

godot_keys() {  # xdotool keys -> Godot names separated by ","
  local out=() k
  for k in $1; do
    case "$k" in Return) k=Enter ;; a) k=A ;; space) k=Space ;; shift+g) k=Shift+G ;; shift+bracketright) k=Shift+BracketRight ;;
      [a-z]) k="${k^^}" ;; esac
    out+=("$k")
  done
  (IFS=,; echo "${out[*]}")
}

field() { echo "$1" | cut -d'|' -f"$2"; }

mode="${1:?godot|godot-ref}"; dir="${2:?folder}"
shift 2
mkdir -p "$dir"

# Godot references for the screens deliberately different from the C (godot option):
# capture then fingerprints DIR/<sequence>.sha1, to be reviewed before versioning them.
if [ "$mode" = godot-ref ]; then
  for s in "${SEQUENCES[@]}"; do
    name="$(field "$s" 1)"; keys="$(field "$s" 2)"; scene="$(field "$s" 3)"
    opt="$(field "$s" 4)"
    [[ "$opt" == godot || "$opt" == godot:* || "$opt" == destruct ]] || continue
    extra=(); [[ "$opt" == godot:* ]] && read -r -a extra <<<"${opt#godot:}"
    mouse="$(field "$s" 5)"; [ -n "$mouse" ] && extra+=("--mouse=$mouse")
    echo "== Godot $name"
    if [ "$opt" = destruct ]; then
      bash "$here/visual_capture_godot.sh" "destruct" "$dir/$name" "" \
        --destruct-ticks=150 --destruct-no-intro=1 --rng-seed=89837 --no-intro >"$dir/$name.log" 2>&1 \
        || { echo "destruct capture failed: $name ($dir/$name.log)" >&2; exit 1; }
      python3 "$here/visual_diff.py" seqhash "$dir/$name" >"$dir/$name.sha1"
      continue
    fi
    bash "$here/visual_capture_godot.sh" "$scene" "$dir/$name" "$(godot_keys "$keys")" "${extra[@]}" >"$dir/$name.log" 2>&1 \
      || { echo "Godot capture failed: $name ($dir/$name.log)" >&2; exit 1; }
    python3 "$here/visual_diff.py" seqhash "$dir/$name" >"$dir/$name.sha1"
  done
  exit 0
fi

ref_c="${1:-}"; ref_g="${2:-}"
shift $(( $# >= 2 ? 2 : $# ))
only=("$@")
jobs_max="${JOBS:-8}"
if [ -z "$ref_c" ] && [ -z "$ref_g" ]; then
  echo "a C or Godot reference is required" >&2; exit 1
fi
pids=(); selected=()

for s in "${SEQUENCES[@]}"; do
  name="$(field "$s" 1)"
  if [ ${#only[@]} -gt 0 ] && [[ " ${only[*]} " != *" $name "* ]]; then continue; fi
  selected+=("$name")
  keys="$(field "$s" 2)"; scene="$(field "$s" 3)"; opt="$(field "$s" 4)"; mouse="$(field "$s" 5)"
  (
    case "$opt" in
      shop:*) bash "$here/visual_capture_godot.sh" shop "$dir/$name" "${opt#shop:}" \
                --shop-loadout=1,1,11,0,1,4,2,0,0,0,14983 --shop-cubes=2 ;;
      seed) bash "$here/visual_capture_godot.sh" "$scene" "$dir/$name" "$(godot_keys "$keys")" --rng-seed=89837 ;;
      day) bash "$here/visual_capture_godot.sh" "$scene" "$dir/$name" "$(godot_keys "$keys")" --unix-time=43200 ;;
      godot:*) read -r -a extra <<<"${opt#godot:}"
               bash "$here/visual_capture_godot.sh" "$scene" "$dir/$name" "$(godot_keys "$keys")" "${extra[@]}" ;;
      destruct) bash "$here/visual_capture_godot.sh" destruct "$dir/$name" "" \
                 --destruct-ticks=150 --destruct-no-intro=1 --rng-seed=89837 --no-intro ;;
      *) bash "$here/visual_capture_godot.sh" "$scene" "$dir/$name" "$(godot_keys "$keys")" ${mouse:+--mouse="$mouse"} ;;
    esac >"$dir/$name.log" 2>&1
  ) &
  pids+=("$!")
  while [ "$(jobs -rp | wc -l)" -ge "$jobs_max" ]; do sleep 1; done
done
fail=0
[ ${#pids[@]} -gt 0 ] || { echo "no sequence selected" >&2; exit 1; }
for i in "${!pids[@]}"; do
  if ! wait "${pids[$i]}"; then
    fail=1
    echo "Godot capture failed: ${selected[$i]} ($dir/${selected[$i]}.log)" >&2
    grep -E 'SCRIPT ERROR:|(^|[[:space:]])ERROR:|leaked at exit|still in use at exit' "$dir/${selected[$i]}.log" >&2 || true
  fi
done

# differing ranks (or "-") between two sequences; the reference is a folder
# of OTFR images or a fingerprint file <sequence>.sha1
cmp_seq() {
  local a="$1" b="$2"; shift 2
  [ -d "$a" ] || { [ -f "$a.sha1" ] && a="$a.sha1"; }
  if [ ! -e "$a" ] || [ ! -d "$b" ]; then echo "ERROR"; return 1; fi
  local result
  if ! result=$(python3 "$here/visual_diff.py" seqdiff "$a" "$b" "$@"); then
    printf '%s\n' "$result" | tail -1 | grep -q 'different ranks=' || { echo "ERROR"; return 1; }
  fi
  printf '%s\n' "$result" | tail -1 | sed -E 's/.*different ranks=([0-9]+).*/\1/'
}

# number of images of a reference (OTFR folder or fingerprint file)
ref_count() {
  if [ -d "$1" ]; then find "$1" -maxdepth 1 -name '*.otfr' ! -name '.*' -printf . | wc -c
  elif [ -f "$1.sha1" ]; then wc -l <"$1.sha1"
  else echo 0; fi
}

printf "| Sequence | C images | Godot images | Ranks ≠ C | Ranks ≠ Godot before |\n|---|---|---|---|---|\n"
for s in "${SEQUENCES[@]}"; do
  name="$(field "$s" 1)"; opt="$(field "$s" 4)"
  [[ " ${selected[*]} " == *" $name "* ]] || continue
  [ -d "$dir/$name" ] || { echo "capture missing: $name" >&2; fail=1; continue; }
  skip=()
  case "$opt" in shop:*) skip=(--skip-c "$SHOP_SKIP_C" --skip-g "$SHOP_SKIP_G" --trim-c) ;; esac
  ng="$(find "$dir/$name" -maxdepth 1 -name '*.otfr' ! -name '.*' -printf . | wc -c)"
  nc="-"; dc="-"; dg="-"
  if [ -n "$ref_c" ]; then
    nc="$(ref_count "$ref_c/$name")"
    dc="$(cmp_seq "$ref_c/$name" "$dir/$name" "${skip[@]}")"
  fi
  if [ -n "$ref_g" ]; then
    dg="$(cmp_seq "$ref_g/$name" "$dir/$name")"
  fi
  for v in "$dc" "$dg"; do [ "$v" = - ] || [ "$v" = 0 ] || fail=1; done
  printf "| %s | %s | %s | %s | %s |\n" "$name" "$nc" "$ng" "$dc" "$dg"
done
exit $fail
