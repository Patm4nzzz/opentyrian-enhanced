# UI regression references

One `<sequence>.sha1` file per sequence of `rewrite/scripts/ui_regression.sh`:
one SHA-1 hash per composed frame (RGB 320x200) of the C version (tag `c-final`), in
display order (`visual_diff.py seqhash`). CI compares every Godot frame
to these hashes (`ui_regression.sh godot OUT golden/ui`) without
running the C version.

These references are FROZEN: the C version is no longer in the repository (ADR 0132),
so they can no longer be regenerated. A screen that changes on purpose
has no C reference left to compare against: it moves to a "godot" sequence (see
below) with a Godot reference reviewed frame by frame.

Godot references: the sequences marked `godot` in `ui_regression.sh`
(Setup and its sub-menus: setup, graphics, sound, gameplay) show
a screen that intentionally differs from the C version (docs/setup-options.md). Their
`<sequence>.sha1` comes from a Godot capture reviewed frame by frame:

```
DISP=:<display> bash rewrite/scripts/ui_regression.sh godot-ref /tmp/ref-g
cp /tmp/ref-g/{setup,graphics,sound,gameplay}.sha1 rewrite/golden/ui/
```

Regenerate only after an intentional change to these screens.

Version label: the title and the screens drawn over it show
`application/config/version` (project.godot) in the bottom-left corner, so
every version change alters those frames. After a version bump (or an
intentional UI change), capture with `ui_regression.sh godot OUT golden/ui`
and rewrite the affected files with `visual_diff.py seqhash OUT/<sequence>`
(shop-1 and shop-2 keep their C prefix and are not rewritten this way).
Last regeneration: v1.0-rc2 (2026-11-23), version label only: the 21
sequences were captured twice on the same build, with the v1.0-rc1 label
(identical to the references, apart from the machine-dependent Display row
of the Graphics menu) then with the v1.0-rc2 label, and only the ranks that
differ between the two captures were rewritten (1,607 ranks in 19 files).
The regeneration before it, v1.0-rc1 (2026-10-20), also took in the
intentional UI changes of 2026-10-10 to 2026-10-12 (quit box colors, mouse
pointer hidden during fades, pause and Graphics menu rows).

Machine-dependent frames: the Display row of the Graphics menu lists the
screens of the machine. The `graphics` reference was captured with one
screen (Xvfb); on a PC with several screens, 6 ranks of that sequence
differ (the row reads "Display 2").

New-game sequences (`new-game`, `difficulty`, `timed-battle`,
`lord`: title_screen.c:newGame, menus.c:gameplaySelect, episodeSelect,
difficultySelect, timedBattleSelect and the Shift+G, Shift+], LORD unlocks):
frozen C references, like the others.
