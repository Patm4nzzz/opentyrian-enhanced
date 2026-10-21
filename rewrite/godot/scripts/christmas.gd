## Christmas mode (opentyr.c, xmas.c).
##
## The C switches to Christmas mode when the local month is December (xmas_time), after
## a Yes / No prompt (xmas_prompt, scene `xmas_prompt`); `--xmas` (-X) enables it
## without a prompt, `--no-xmas` (-x) disables it. Once enabled, it replaces tyrian.shp with
## tyrianc.shp (redrawn shots, ships and bonuses: sheets 7, 8, 9 and 11) and
## voices.snd with voicesc.snd (voices 32 to 40). If either file is missing,
## the mode stays off (opentyr.c: "christmas data missing").
##
## Outside December, with no option, nothing changes: `active` stays false.
## To try the prompt out of season, `--unix-time=<seconds>` (existing capture option,
## like time() under fixed_seed_shim.c) sets the date read here.
class_name Christmas
extends RefCounted

## Sheets that tyrianc.shp redraws -> imported Christmas sheet (imported/sprites).
const SHEETS := {
	"main_07_player_shots": "main_christmas_07_player_shots",
	"main_08_player_ships": "main_christmas_08_player_ships",
	"main_09_powerups": "main_christmas_09_powerups",
	"main_11_player_shots2": "main_christmas_11_player_shots2",
}
## Voice sample numbers (tyrian.snd has 31, voices.snd 9 more).
const VOICE_FIRST := 32
const VOICE_LAST := 40

## opentyr.c: xmas.
static var active := false
## Answer already given (or forced by an option): the prompt does not come back.
static var decided := false


## xmas.c:xmas_time: the local month is December.
static func in_season() -> bool:
	var month: int
	if Capture.has_option("unix-time"):
		var bias: int = Time.get_time_zone_from_system()["bias"]
		month = int(Time.get_datetime_dict_from_unix_time(int(Capture._options["unix-time"]) + bias * 60)["month"])
	else:
		month = int(Time.get_datetime_dict_from_system(false)["month"])
	return month == 12


## 1: `--xmas` / `-X`, 0: `--no-xmas` / `-x`, -1: no option (params.c).
static func forced_state() -> int:
	var args := OS.get_cmdline_user_args()
	if args.has("--no-xmas") or args.has("-x"):
		return 0
	if args.has("--xmas") or args.has("-X"):
		return 1
	return -1


## opentyr.c: the Christmas files must exist, otherwise the mode stays off.
## Here: tyrianc.shp in the data folder (read by TyrianVga) and the Christmas voices
## imported from voicesc.snd (the game reads the imported WAVs, not the .snd).
static func data_available() -> bool:
	return (
		FileAccess.file_exists(Game.data_dir.path_join("tyrianc.shp"))
		and ResourceLoader.exists(TyrianAssets.IMPORTED.path_join(sfx_path_christmas(VOICE_FIRST)))
	)


## Command-line options, read at launch (Game._ready).
static func apply_command_line() -> void:
	var forced := forced_state()
	if forced < 0:
		return
	decided = true
	set_active(forced == 1)


## True when the title must first show the prompt: December, no option,
## Christmas files present, prompt not yet shown. Never under the capture
## tool (screen requested by name: `xmas_prompt`).
static func needs_prompt() -> bool:
	return not decided and not Capture.active and in_season() and data_available()


## Enables or disables the mode: reloads the shape table resources
## (`TyrianVga`), clears the cached sheets and re-reads the voices.
static func set_active(on: bool) -> void:
	if on and not data_available():
		push_warning("Christmas data missing")
		on = false
	if on == active:
		return
	active = on
	ClassDB.class_call_static("TyrianVga", "set_christmas", Game.data_dir, on)
	var tree := Engine.get_main_loop() as SceneTree
	var sfx: Node = tree.root.get_node_or_null("Sfx") if tree != null else null
	if sfx != null:
		sfx.reload_voices()


## Path (under imported/) of sample `number`: the Christmas voices replace the regular ones.
static func sfx_path(number: int) -> String:
	if active and number >= VOICE_FIRST and number <= VOICE_LAST:
		return sfx_path_christmas(number)
	return "sfx/%d.wav" % number


static func sfx_path_christmas(number: int) -> String:
	return "sfx/christmas/%d.wav" % number
