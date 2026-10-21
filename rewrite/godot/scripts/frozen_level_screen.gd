## 8-bit screen laid over the frozen image of a level (mainint.c:
## JE_inGameSetup and JE_inGameHelp draw into VGAScreenSeg, which holds
## the last presented image: play area copied from game_screen by
## JE_starShowVGA, dashboard around it, texts of JE_inGameDisplays).
##
## The image is recomposed in 8-bit indices: play area by
## TyrianSim.playfield_indices, dashboard frame and texts by the
## level's HudRenderer surfaces, level palette (pic 3, or 6 in two-player
## mode). The simulation is not touched.
extends TyrianScreen

## enemy_render.c:JE_starShowVGA: game_screen (x 24..287) -> VGAScreenSeg (x 0..263).
const PLAY_AREA_X := 24
const PLAY_W := 264
const PLAY_H := 184

var level: Node
## True when the screen is waiting for a key (capture tool).
var idle := false


## Puts the frozen level image into VGAScreen; false if it is not
## available (no extension, no 8-bit dashboard).
func freeze(lvl: Node) -> bool:
	level = lvl
	var hud: HudRenderer = lvl.get("_hud_vga")
	var sim: Object = lvl.get("sim")
	if vga == null or hud == null or hud.vga == null or sim == null or not sim.has_method("playfield_indices"):
		return false
	var pf: PackedByteArray = sim.playfield_indices(Game.data_dir)
	if pf.size() != 320 * 200:
		return false
	# Level palette (tyrian2.c:JE_main: JE_loadPic(3 or 6) then fade_palette).
	var two: bool = lvl.call("_two_players")
	vga.load_pic(VGA_SCREEN2, HudRenderer.PIC_TWO_PLAYERS if two else HudRenderer.PIC_ONE_PLAYER, true)
	var seg: PackedByteArray = hud.vga.indices(HudRenderer.SEG)
	var a: PackedByteArray = hud.vga.indices(HudRenderer.OVER_A)
	var b: PackedByteArray = hud.vga.indices(HudRenderer.OVER_B)
	for y in PLAY_H:
		var row := y * 320
		for x in PLAY_W:
			var j := row + x + PLAY_AREA_X
			# JE_inGameDisplays (equal pixels on both witness surfaces)
			# over the play area.
			seg[row + x] = a[j] if a[j] == b[j] else pf[j]
	vga.set_indices(VGA_SCREEN, seg)
	return true
