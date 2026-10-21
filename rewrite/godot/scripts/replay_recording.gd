## Recording of a `.rep` replay during a game (replay.c:
## replay_start_recording, begin_recording, replay_stop_recording).
##
## As in the C code, the key does not start recording right away:
## outside a recording it ARMS (or disarms) it for the next start of a
## level ("Recording at next level"); during a recording
## it closes it and writes the file. A recording also closes at the end
## of the level (tyrian2.c, start_level) or when the player leaves it. The file is
## written to the user's `replays/` folder under the timestamped name
## YYYYMMDD-HHMMSS.rep, which the Replays screen lists.
##
## The simulation (TyrianSim.request_recording / finish_recording, crate
## tyrian-sim: ReplayRecorder) writes the same format as the C code; here only
## the key, the messages and the file.
class_name ReplayRecording
extends RefCounted

## replay.c:s_rec_armed: recording requested for the next level.
static var armed := false


## Replay folder: `--replay-dir=` (captures and tests), otherwise `replays/`
## of the user folder (replay.c:ensure_replay_dir).
static func directory() -> String:
	if Capture.has_option("replay-dir"):
		return String(Capture._options["replay-dir"])
	return UserData.path("replays")


## replay.c:replay_stop_recording: "%Y%m%d-%H%M%S.rep" in local time.
## Two recordings in the same second: the second takes a suffix
## rather than overwriting the first.
static func file_path(now := Time.get_datetime_dict_from_system()) -> String:
	var stem := "%04d%02d%02d-%02d%02d%02d" % [now.year, now.month, now.day, now.hour, now.minute, now.second]
	var dir := directory()
	var path := dir.path_join(stem + ".rep")
	var n := 2
	while FileAccess.file_exists(path):
		path = dir.path_join("%s-%d.rep" % [stem, n])
		n += 1
	return path


## Writes the file of a closed recording; returns its path, "" on failure
## (empty bytes: nothing was recorded).
static func save(bytes: PackedByteArray) -> String:
	if bytes.is_empty():
		return ""
	var dir := directory()
	if DirAccess.make_dir_recursive_absolute(dir) != OK:
		push_error("Replay: cannot create folder %s" % dir)
		return ""
	var path := file_path()
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		push_error("Replay: cannot write %s" % path)
		return ""
	f.store_buffer(bytes)
	f.close()
	return path


## replay.c:replay_start_recording: arms, or disarms if already armed. Returns the
## locale key of the message to display.
static func toggle_arm() -> String:
	armed = not armed
	return "msg_replay_armed" if armed else "msg_replay_disarmed"
