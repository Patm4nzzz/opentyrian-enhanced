# ADR 0084 — Godot tests with gdUnit4

Status: accepted (2026-10-05).

## Context

The Godot-side checks were five homegrown scripts (`tools/check_scripts.gd`,
`check_controls.gd`, `check_endless.gd`, `check_modes.gd`, `campaign_check.gd` + `.tscn`), each
with its own `_check`, its own output format and its own exit code. CI
ran four of them; `check_modes` was run nowhere and asserted almost nothing (it printed
the values instead of checking them).

## Decision

- gdUnit4 **6.2.1** (godot-gdunit-labs/gdUnit4, MIT; compatible with Godot 4.5 to 4.7.x, the project
  is on 4.7.2) copied into `rewrite/godot/addons/gdUnit4/` without its own tests. Excluded from
  export like the other addons; the `test/` folder is too.
- Suites in `rewrite/godot/test/*_test.gd`, one per former script:

  | Former script | Checks executed | gdUnit4 suite | Cases | Checks |
  |---|---|---|---|---|
  | `check_scripts.gd` | 1 per script (43) | `scripts_test.gd` | 1 | 1 per script + "> 30 scripts" guard |
  | `check_controls.gd` | 22 | `controls_test.gd` | 6 | all 22, same values |
  | `check_endless.gd` | 12 | `endless_test.gd` | 1 | all 12, same order |
  | `campaign_check.gd` | 26 | `campaign_test.gd` | 4 | all 26 (counted at run time) |
  | `check_modes.gd` | 1 (+ printed values) | `modes_test.gd` | 1 | 7: printed values turned into assertions |

  13 test cases in total. The former scripts are deleted.
- Launcher `rewrite/scripts/godot-tests.sh`: headless, temporary user folder (the player's
  preferences, saves and mods do not influence the tests), and goes through
  `run_godot_checked`: a Godot `SCRIPT ERROR` / `ERROR` makes it fail even if gdUnit4 returns 0.
  It also fails on an orphan node (gdUnit4 code 101) and if no test is run.
- `--ignoreHeadlessMode`: the tests inject their inputs through `Input.parse_input_event`, which
  works without a display; none depends on the interface events that Godot does not carry
  in headless mode.
- CI (`rewrite.yml`): "Godot tests (gdUnit4)" step after the build, HTML/XML report
  published as an artifact; the workflow keeps its `concurrency` (cancelling the previous run).

## Evidence (2026-10-05, WSL, Godot 4.7.2)

- Before: the five scripts green (22 + 12 + 26 checks, 43 scripts), three passes.
- After: 13 cases, 0 failures, 0 orphans, no ERROR line, three passes.
- Mutations, each failing: a game script that no longer compiles (exit 100), a modified
  Steam Deck profile (exit 100, `test_steam_deck_profile`), a node forgotten by a test (exit 101, orphan),
  an empty test folder (exit 1, "no test run").
- Defect fixed along the way: resuming a C save (`load_campaign_slot`) left the old
  scene to be freed after the test ended (4 orphans); the test now waits for a frame.

## Consequences

- A new test = a `test_*` method in `rewrite/godot/test/`. Run:
  `GODOT=godot bash rewrite/scripts/godot-tests.sh [report-folder]`.
- Updating gdUnit4: replace the addon folder with the new version (without `test/`)
  after checking its Godot compatibility table.
