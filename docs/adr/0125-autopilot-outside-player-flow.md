# ADR 0125 — Autopilot outside the player flow

Date: 2026-10-04 (block 20.5, refactoring approved by the maintainer)

## Context

The C version has two pieces of autopilot (`autopilot.c`):
`autopilot_buy_upgrades` (shop purchases between two levels) and
`autopilot_drive` (in-flight assists, replayed by `apply_assist` when a golden
master is recorded with `assist 1`). The campaign batch (round 3 bis) already
replaced the automatic purchases with the interactive shop (`shop.tscn`,
`TyrianShop`); on the Godot side, there remained a game-state field
`Game.assist` read by the level scene on every launch: any
screen of the player flow could enable it, and its value survived from one game to the
next in the autoload.

## Findings (grep)

- `autopilot_buy_upgrades` exists in no Rust or GDScript code; it is only
  cited in a comment by `tyrian-golden` (test `campaign.rs`, example
  `shop_probe.rs`) to explain the money in the C golden masters.
- `tyrian-godot` only exposes `TyrianSim.set_assist` (default false).
- Only the tools enabled the assists: `tools/capture.gd` (`assist 1` header
  of a `--replay=`), `tools/bench.gd` (measurement), and
  `tools/campaign_check.gd` (Sim instantiated directly; now `test/campaign_test.gd`, ADR 0084).

## Decision

- `Game.assist` is deleted. The level scene computes its assists from the
  replay tools alone (`Capture.replay_assist`, `Bench.replay_assist`,
  both false without the `--capture=… --replay=` / `--bench` option) and keeps them
  in its own `assist` field, readable by the checks.
- `TyrianSim.set_assist` remains: it is the API of the golden masters and tests.
- `campaign_check` proves that a new game opens the interactive shop scene
  and then a level scene whose `assist` is false.

## Consequences

The autopilot is no longer reachable from a menu or a game;
only tools launched with explicit options enable it. Golden masters
66/66 and replay captures "IDENTICAL" unchanged.
