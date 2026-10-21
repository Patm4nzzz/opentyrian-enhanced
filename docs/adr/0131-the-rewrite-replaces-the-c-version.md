# ADR 0131 — The Godot + Rust rewrite replaces the C version

Status: accepted (2026-10-05, decision by the maintainer).

## Context
OpenTyrian2000 Enhanced used to exist in C. The rewrite (simulation in Rust, display, interface and editor in Godot 4) plays the full campaign, the side modes, ALE and SQUADRON, and remains identical to the C version on 67 strict references. Since ADR 0127, the references are written by the rewrite itself.

## Decision
1. The Godot + Rust version becomes the project's main version; the README and the GitHub presentation present it as such, aimed at new players.
2. The C version is temporarily kept as a comparison reference, until the first final release.
3. After that release: merge of PR #7, then removal of the C version from the repository (decision and schedule to be confirmed by the maintainer).
4. A bug inherited from the C version is fixed in the rewrite, never reproduced through a compatibility mode.

## Consequences
- New features are developed only in the rewrite; the C version only receives the fixes needed for the comparison.
- The packages published for players are those of the rewrite (OpenTyrianGodot-*).
