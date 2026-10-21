## Music (autoload `Music`): playback of the OGGs rendered offline from
## music.mus by the C OPL code (rewrite/tools/music-render, ADR 0004 and
## 0010) with the DOSBox emulator (Classic) or Nuked OPL3 (Authentic,
## assets/imported/music_nuked), or, at the player's choice (Setup > Sound,
## `engine`, sound_menu.c:eng_set), the tracks
## data/midi/NN-*.mid played with data/soundfont.sf2 by the extension's
## synth (TyrianMidi, rustysynth, ADR 0130) like the C Enhanced
## engine. A missing or unreadable MIDI track, or a missing SoundFont, replays
## the OPL version with a warning (loudness.c:play_song, midi_fallback).
## Reproduces the loudness.c interface as seen by the game:
## play_song / restart_song / stop_song / fade_song / set_volume /
## fade_and_play_song, and the `playing` state of lds_play.c.
##
## Track numbers are 0-based like `play_song` (title: 29).
## Nothing here is read by the simulation.
extends Node

const MUSIC_DIR := "res://assets/imported/music"
## Nuked OPL3 render (Authentic engine, `MUSIC_BACKEND=nuked` of render.sh).
const MUSIC_NUKED_DIR := "res://assets/imported/music_nuked"
## Bus of the OPL output (Music Widen) and of the MIDI synth (Stereo off).
const BUS_OPL := &"MusicOpl"
const BUS_MIDI := &"MusicMidi"

## musmast.h
const SONG_BUY_DEFAULT := 2  # DEFAULT_SONG_BUY (level_load.c:JE_loadMap, songBuy)
const SONG_CREDITS := 8  # mainint.c:JE_playCredits, play_song(8)
const SONG_LEVELEND := 9
const SONG_GAMEOVER := 10
const SONG_SUPER_TYRIAN := 16  # title_screen.c:newSuperTyrianGame, play_song(16)
const SONG_MAPVIEW := 19  # mainint.c:JE_helpSystem
const SONG_NEXT_EPISODE := 26  # mainint.c:JE_nextEpisode, play_song(26)
const SONG_TITLE := 29
const SONG_ZANAC := 31  # highscore.c:JE_highScoreCheck, play_song(31) after the entry
const SONG_HIGH_SCORES := 33  # highscore.c:JE_highScoreCheck, play_song(33)

## Music engine (`engine`, Setup's `music_engine` setting), numbering
## of sound_menu.c:eng_get: 0 Classic (DOSBox OPL), 1 Authentic (Nuked
## OPL3), 2 Enhanced (MIDI + SoundFont).
const ENGINE_CLASSIC := 0
const ENGINE_AUTHENTIC := 1
const ENGINE_ENHANCED := 2
const ENGINE_COUNT := 3
## Old names: Original OPL = Classic, MIDI = Enhanced.
const ENGINE_OPL := ENGINE_CLASSIC
const ENGINE_MIDI := ENGINE_ENHANCED

## Gain difference between engines: loudness.c doubles the volume factor
## of the OPL only ("OPL is too quiet"); MIDI keeps the table alone.
const OPL_GAIN_DB := 6.0206

## Reserve of the MIDI generator: enough for a slow frame without a gap, short
## enough that a track change is heard right away.
const MIDI_BUFFER_S := 0.25

## Default tyrMusicVolume (config.c:735).
const DEFAULT_VOLUME := 191

## Envelope of fade_song() (lds_fade(1)) measured on the tool's render
## (verify.py fade, tracks 29 and 17): about -11.5 dB/s, below -40 dB in
## 3.5 to 3.6 s; allvolume starts at 0 (= full volume), goes to 255 then
## decreases by 1 per lds_update at 69.5 Hz: 255 / 69.5 = 3.67 s, after which
## the track keeps going at allvolume 1 (almost silent) until the next play_song.
const FADE_SONG_DB_PER_S := 11.5
const FADE_SONG_SECONDS := 255.0 / 69.5

## fade_and_play_song (loudness.c:music_transition_update): step of 8 on
## musicVolume every 20 ms, safety cutoff at 2 s.
const TRANSITION_STEP_MS := 20
const TRANSITION_STEP := 8
const TRANSITION_TIMEOUT_MS := 2000

## Index of the last requested track (loudness.c:song_playing), -1 = none.
var song_playing := -1
## jukebox.c:ingame_song_override: track chosen with G in the Jukebox,
## played at the start of each level instead of levelSong (-1: none).
var ingame_song_override := -1
## Previous MIDI position (seconds): a step back = the track has looped.
var _midi_last_position := 0.0
var _midi_looped := false
## loudness.c:music_stopped
var music_stopped := true
## Player setting (tyrMusicVolume, 0..255).
var user_volume := DEFAULT_VOLUME
## loudness.c:musicVolume (0..255): current volume, changed by set_volume.
var music_volume := 255
var enabled := true
## Last requested pause state (track stop, level pause), excluding mute.
var _pause_requested := false
## Setup > Sound: music off (loudness.c:music_disabled).
var muted := false:
	set(v):
		muted = v
		# loudness.c:audioCallback no longer pulls music when music_disabled:
		# the track freezes and resumes at the same point.
		if _player != null:
			_set_stream_paused(_pause_requested)
		_apply_volume()
## --music-log: traces every order on standard output (proofs, tests).
var log_enabled := false
## True after release_streams: no track is loaded any more before shutdown.
var _released := false

## Engine chosen by the player (ENGINE_*). The choice stays recorded even if
## the SoundFont or the Nuked render are missing (sound_menu.c:
## music_backend_preference); each track then falls back to Classic.
var engine := ENGINE_CLASSIC
## Setup > Sound > Music Widen (loudness.c:music_pseudo_stereo): pseudo-stereo
## (ps_widen) on the OPL output, TyrianPseudoStereo effect of the BUS_OPL bus.
## The effect is only enabled / disabled: replacing it (remove_bus_effect then
## add_bus_effect) waits for the audio thread and froze the main loop. The
## 25-sample delay keeps at most 0.6 ms of old signal at
## activation, where the C code zeroes the ring.
var widen := false:
	set(v):
		widen = v
		_set_bus_effect(BUS_OPL, widen)
## Setup > Sound > Stereo (loudness.c:stereo_enabled): off, the MIDI is
## brought down to mono ("downmix to dual-mono"); the OPL is mono anyway.
var stereo := true:
	set(v):
		stereo = v
		_set_bus_effect(BUS_MIDI, not stereo)
## Tracks of the Nuked render (music_nuked/music.json), empty if missing.
var _nuked_songs: Array = []
## data/ folder of the MIDIs and the SoundFont; empty = Game.data_dir (tests).
var midi_data_dir := ""

var _songs: Array = []
var _player: AudioStreamPlayer
## MIDI synth player: AudioStreamGenerator filled in _process.
var _midi_player: AudioStreamPlayer
var _midi: TyrianMidi
## SoundFont loaded from that folder ("": not yet tried or missing).
var _midi_loaded_from := ""
## True when the current track goes through the MIDI synth.
var _midi_song := false
var _streams := {}
## fade_song fade: seconds elapsed, -1 = inactive.
var _fade_elapsed := -1.0
## fade_and_play_song transition: 0 none, 1 fade down, 2 fade up.
var _transition := 0
var _pending_song := 0
var _saved_volume := 255
var _step_timer_ms := 0.0
var _transition_elapsed_ms := 0.0


func _ready() -> void:
	_player = AudioStreamPlayer.new()
	_player.name = "MusicPlayer"
	_player.bus = _ensure_bus(BUS_OPL, _new_widen_effect())
	add_child(_player)
	_midi_player = AudioStreamPlayer.new()
	_midi_player.name = "MidiPlayer"
	_midi_player.bus = _ensure_bus(BUS_MIDI, _new_mono_effect())
	var gen := AudioStreamGenerator.new()
	gen.mix_rate = TyrianMidi.SAMPLE_RATE
	gen.buffer_length = MIDI_BUFFER_S
	_midi_player.stream = gen
	add_child(_midi_player)
	var text := FileAccess.get_file_as_string(MUSIC_DIR.path_join("music.json"))
	if text.is_empty():
		push_warning("Music: %s/music.json missing (run rewrite/tools/music-render/render.sh): music muted" % MUSIC_DIR)
	else:
		var data: Variant = JSON.parse_string(text)
		if data is Dictionary:
			_songs = data.get("songs", [])
	var nuked: Variant = JSON.parse_string(FileAccess.get_file_as_string(MUSIC_NUKED_DIR.path_join("music.json")))
	if nuked is Dictionary:
		_nuked_songs = nuked.get("songs", [])
	for arg in OS.get_cmdline_user_args():
		if arg == "--no-music" or arg == "-s" or arg == "--no-sound":
			enabled = false
		elif arg == "--music-log":
			log_enabled = true
	_apply_volume()


## musmast.c:musicTitle (MUSIC_NUM = 41). The number and titles of the
## tracks are the C's, whether the OPL render (music.json) is present or not:
## without it, the Jukebox navigates and displays the titles like the muted C
## (audio_disabled), instead of an empty list.
const MUSIC_TITLES := [
	"Asteroid Dance, Part 2",
	"Asteroid Dance, Part 1",
	"Buy/Sell Screen",
	"Camanis",
	"Camanis Boss",
	"Deliani",
	"Deliani Alternate",
	"Ending 1",
	"Ending 2",
	"End of Level",
	"Game Over",
	"Gryphon",
	"Gryphon Boss",
	"Gyges",
	"Gyges Boss",
	"Hallows",
	"Tunnel",
	"Tyrian, the Level",
	"The Musicman",
	"The Navigator",
	"Savara",
	"Savara Boss",
	"Space Battle 1",
	"Space Battle 2",
	"Ending 3",
	"Secret Level",
	"Story Mode",
	"Torm",
	"Transon Lohk",
	"Tyrian - Main Title",
	"Power Up",
	"Secret Level 2",
	"Savara, the Return",
	"High Scores",
	"One Must Fall Tribute",
	"Sarah's Song",
	"A Field for Mag",
	"Rock Garden",
	"Quest for Peace",
	"Composition in Q",
	"Beer",
]


## musmast.h:MUSIC_NUM.
func song_count() -> int:
	return MUSIC_TITLES.size()


func title(song: int) -> String:
	return str(MUSIC_TITLES[song]) if song >= 0 and song < MUSIC_TITLES.size() else ""


## Track render available (music.json imported). False: music muted,
## like loudness.c with audio_disabled.
func has_audio() -> bool:
	return not _songs.is_empty()


## loudness.c:play_song — only reloads if the track changes; otherwise
## simply resumes (music_stopped = false).
func play_song(song: int) -> void:
	_log("play_song %d (%s)%s" % [song, title(song), "" if song != song_playing else " : already loaded, resuming"])
	if song != song_playing:
		_load(song)
		song_playing = song
	music_stopped = false
	_set_stream_paused(false)


## loudness.c:restart_song (lds_rewind: allvolume reset to zero).
func restart_song() -> void:
	_log("restart_song")
	_fade_elapsed = -1.0
	if _midi_song:
		# loudness.c:restart_song: the player in charge restarts the
		# track; if it can no longer (file removed), OPL version.
		if not _play_midi(song_playing):
			_load(song_playing)
	elif _player.stream != null:
		_player.play(0.0)
	music_stopped = false
	_set_stream_paused(false)
	_apply_volume()


## loudness.c:stop_song — the player is suspended, not rewound.
func stop_song() -> void:
	_log("stop_song")
	music_stopped = true
	_set_stream_paused(true)


## Pause of a level under an options screen (F9 / F10, Effects.open_options):
## the music suspends and resumes where it was, unless the game had stopped it.
func set_paused(paused: bool) -> void:
	if _player != null:
		_set_stream_paused(paused or music_stopped)


func _set_stream_paused(paused: bool) -> void:
	_pause_requested = paused
	_player.stream_paused = paused or muted
	_midi_player.stream_paused = paused or muted


## Program shutdown: releases the stream being played. Without this, the OGG playback
## still held by the audio server is reported as "resources still in use".
func release_streams() -> void:
	_log("release_streams")
	_transition = 0
	_fade_elapsed = -1.0
	_released = true
	_player.stop()
	_player.stream = null
	_streams.clear()
	_stop_midi()
	song_playing = -1
	music_stopped = true


## loudness.c:fade_song (lds_fade(1)).
func fade_song() -> void:
	_log("fade_song")
	if _fade_elapsed < 0.0:
		_fade_elapsed = 0.0


## loudness.c:set_volume (music volume only; effects have Sfx).
func set_volume(volume: int) -> void:
	if volume != music_volume:
		_log("set_volume %d" % volume)
	music_volume = clampi(volume, 0, 255)
	_apply_volume()


## loudness.c:fade_and_play_song: nothing if the track is already playing; direct
## start if the music is stopped; otherwise volume fade down, change,
## fade up (music_transition_update).
func fade_and_play_song(song: int) -> void:
	_log("fade_and_play_song %d (%s)" % [song, title(song)])
	if song == song_playing and not music_stopped:
		return
	if music_stopped:
		play_song(song)
		return
	_pending_song = song
	_saved_volume = music_volume
	_transition = 1
	_step_timer_ms = 0.0
	_transition_elapsed_ms = 0.0


## lds_play.c:playing — false once the end command (0xfc) is reached
## for a non-looping track; a looping track always plays.
func is_playing() -> bool:
	if _midi_song:
		return not _midi.is_done()
	if song_playing < 0 or song_playing >= _songs.size() or _player.stream == null:
		return false
	var s: Dictionary = _songs[song_playing]
	var end_frame := int(s.get("end_frame", -1))
	if bool(s["loop"]) or end_frame < 0:
		return true
	return _player.playing and _player.get_playback_position() * float(s["rate"]) < end_frame


## lds_play.c:songlooped (midi_has_looped in MIDI): the current track has
## finished a first pass and resumed at its loop point. The OGG render
## (music_render.c) places loop_start at the C's first backward jump:
## the position has therefore passed loop_start as soon as the C would have looped.
func song_looped() -> bool:
	if _midi_song:
		var pos := _midi.position()
		if pos < _midi_last_position:
			_midi_looped = true
		_midi_last_position = pos
		return _midi_looped
	if song_playing < 0 or song_playing >= _songs.size() or _player.stream == null or not _player.playing:
		return false
	var s: Dictionary = _songs[song_playing]
	return bool(s["loop"]) and _player.get_playback_position() >= float(s["loop_start"])


## Executes the orders of TyrianSim.take_music_cues() (kind, value pairs;
## kind 0 = play_song, 1 = set_volume), in the C's order.
func apply_cues(cues: PackedInt32Array) -> void:
	var i := 0
	while i + 1 < cues.size():
		match cues[i]:
			0:
				play_song(cues[i + 1])
			1:
				set_volume(cues[i + 1])
		i += 2


## Track ]L[ (levelSong, 1-based in the episode file) of level
## `lvl_file_num`; `section` > 0 disambiguates a reused level file
## (ep1 no. 5, ep5 no. 3 and 4). 0 if unknown.
static func level_song(episode: int, lvl_file_num: int, section := 0) -> int:
	var ep: Variant = TyrianAssets.read_json("episodes/ep%d.json" % episode)
	if not ep is Dictionary:
		return 0
	var first := 0
	for sec in ep["sections"]:
		for l in sec["levels"]:
			if int(l["lvl_file_num"]) != lvl_file_num:
				continue
			if section > 0 and int(sec["number"]) == section:
				return int(l["level_song"])
			if first == 0:
				first = int(l["level_song"])
	return first


## songBuy of the section (level_load.c:JE_loadMap): DEFAULT_SONG_BUY, or the
## value of a ]i[ in the section (songBuy = atoi(s + 3) - 1).
static func shop_song(episode: int, section: int) -> int:
	var ep: Variant = TyrianAssets.read_json("episodes/ep%d.json" % episode)
	var song := SONG_BUY_DEFAULT
	if ep is Dictionary:
		for sec in ep["sections"]:
			if int(sec["number"]) != section:
				continue
			for line in sec["lines"]:
				var s := str(line)
				if s.length() > 3 and s[0] == "]" and s[1] == "i":
					song = int(s.substr(3).strip_edges().get_slice(" ", 0)) - 1
	return song


func _process(delta: float) -> void:
	if _midi_song:
		_fill_midi()
	if _fade_elapsed >= 0.0 and not music_stopped:
		_fade_elapsed += delta
		_apply_volume()
	if _transition != 0:
		_transition_update(delta * 1000.0)


func _transition_update(frame_ms: float) -> void:
	_transition_elapsed_ms += frame_ms
	if _transition_elapsed_ms >= TRANSITION_TIMEOUT_MS:
		if _transition == 1:
			play_song(_pending_song)
		set_volume(_saved_volume)
		_transition = 0
		return
	_step_timer_ms += frame_ms
	while _step_timer_ms >= TRANSITION_STEP_MS and _transition != 0:
		_step_timer_ms -= TRANSITION_STEP_MS
		if _transition == 1:
			if music_volume > TRANSITION_STEP:
				set_volume(music_volume - TRANSITION_STEP)
			else:
				set_volume(0)
				play_song(_pending_song)
				_transition = 2
				_step_timer_ms = 0.0
		else:
			if music_volume + TRANSITION_STEP < _saved_volume:
				set_volume(music_volume + TRANSITION_STEP)
			else:
				set_volume(_saved_volume)
				_transition = 0


func _load(song: int) -> void:
	if _released:
		return
	_fade_elapsed = -1.0
	if song < 0 or song >= _songs.size():
		# loudness.c:load_song: "failed to load song", the player keeps
		# the previous track.
		push_warning("Music: song %d is outside music.mus" % (song + 1))
		return
	_stop_midi()
	_midi_last_position = 0.0
	_midi_looped = false
	if engine == ENGINE_ENHANCED and enabled:
		_midi_song = _play_midi(song)
		if _midi_song:
			_player.stop()
			_apply_volume()
			return
		push_warning("Music: song %d has no playable MIDI, played the OPL version instead" % (song + 1))
	var stream := _stream(song, engine == ENGINE_AUTHENTIC)
	if stream == null and engine == ENGINE_AUTHENTIC:
		push_warning(
			"Music: song %d has no Nuked render (%s), played the Classic version instead" % [song + 1, MUSIC_NUKED_DIR]
		)
		stream = _stream(song, false)
	_player.stream = stream
	if stream != null and enabled:
		_player.play(0.0)
	_apply_volume()


## OGG stream of a track: DOSBox render (Classic) or Nuked (Authentic);
## null if the requested render is missing.
func _stream(song: int, nuked := false) -> AudioStream:
	var key := song + (1000 if nuked else 0)
	if _streams.has(key):
		return _streams[key]
	var table: Array = _nuked_songs if nuked else _songs
	if song < 0 or song >= table.size():
		return null
	var s: Dictionary = table[song]
	var path := (MUSIC_NUKED_DIR if nuked else MUSIC_DIR).path_join(str(s["file"]))
	var stream: AudioStreamOggVorbis = null
	if ResourceLoader.exists(path):
		stream = load(path) as AudioStreamOggVorbis
	if stream == null:
		stream = AudioStreamOggVorbis.load_from_file(ProjectSettings.globalize_path(path))
	if stream == null:
		push_warning("Music: %s unreadable" % path)
		return null
	# Loop computed by music_render (end of pass -> loop_start).
	stream.loop = bool(s["loop"])
	stream.loop_offset = float(s["loop_start"]) if stream.loop else 0.0
	_streams[key] = stream
	return stream


## True if the Nuked render (Authentic engine) is present.
func has_authentic() -> bool:
	return not _nuked_songs.is_empty()


## Bus `bus_name` sent to Master with its (disabled) effect at the head.
func _ensure_bus(bus_name: StringName, effect: AudioEffect) -> StringName:
	var idx := AudioServer.get_bus_index(bus_name)
	if idx < 0:
		AudioServer.add_bus()
		idx = AudioServer.bus_count - 1
		AudioServer.set_bus_name(idx, bus_name)
		AudioServer.set_bus_send(idx, &"Master")
	if effect != null:
		AudioServer.add_bus_effect(idx, effect, 0)
		AudioServer.set_bus_effect_enabled(idx, 0, false)
	return bus_name


func _set_bus_effect(bus_name: StringName, on: bool) -> void:
	var idx := AudioServer.get_bus_index(bus_name)
	if idx >= 0 and AudioServer.get_bus_effect_count(idx) > 0:
		AudioServer.set_bus_effect_enabled(idx, 0, on)


## loudness.c:ps_widen (TyrianPseudoStereo extension), null without the extension.
static func _new_widen_effect() -> AudioEffect:
	if not ClassDB.class_exists(&"TyrianPseudoStereo"):
		return null
	return ClassDB.instantiate(&"TyrianPseudoStereo")


## Mono: center of both channels (StereoEnhance, spacing 0, no delay).
static func _new_mono_effect() -> AudioEffect:
	var mono := AudioEffectStereoEnhance.new()
	mono.pan_pullout = 0.0
	mono.time_pullout_ms = 0.0
	mono.surround = 0.0
	return mono


## Engine choice (Setup > Sound): the current track restarts right away
## in the chosen engine, from the start, as after sound_menu.c:eng_set
## (stop_song then play_song), and stays suspended if the music was.
func set_engine(value: int) -> void:
	value = value if value >= 0 and value < ENGINE_COUNT else ENGINE_CLASSIC
	if value == engine:
		return
	engine = value
	_log("set_engine %s" % ["classic", "authentic", "enhanced"][engine])
	if song_playing < 0 or _released:
		return
	var song := song_playing
	var stopped := music_stopped
	_transition = 0
	_load(song)
	if stopped:
		_set_stream_paused(true)


## True if the current track is played by the MIDI synth (otherwise by the OPL,
## chosen or as a fallback).
func is_midi_playing() -> bool:
	return _midi_song


func _midi_dir() -> String:
	return midi_data_dir if not midi_data_dir.is_empty() else Game.data_dir


## Synth ready (SoundFont loaded from the current folder); a missing
## SoundFont is retried at the next track, like midi_init at each choice.
func _midi_ready() -> bool:
	var dir := _midi_dir()
	if _midi != null and _midi_loaded_from == dir:
		return true
	_midi = TyrianMidi.new()
	_midi_loaded_from = ""
	var font := dir.path_join("soundfont.sf2")
	if not _midi.load_soundfont(font):
		push_warning("Music: SoundFont %s missing or unreadable, using OPL music" % font)
		_midi = null
		return false
	_midi_loaded_from = dir
	return true


## midi_music.c:midi_play_song: "NN-" file of the track; loops like
## the OPL render (music.json: resume at loop_start - pass, i.e. the same
## instant of the track), only once for a non-looping track.
func _play_midi(song: int) -> bool:
	if song < 0 or song >= _songs.size() or not _midi_ready():
		return false
	var file := TyrianMidi.song_file(_midi_dir().path_join("midi"), song)
	if file.is_empty():
		return false
	var s: Dictionary = _songs[song]
	var looping := bool(s["loop"])
	var loop_from := 0.0
	if looping:
		var lap := float(s["duration"]) - float(s["loop_start"])
		loop_from = maxf(float(s["loop_start"]) - lap, 0.0)
	if not _midi.play_file(file, looping, loop_from):
		return false
	_log("midi %s" % file.get_file())
	# New buffer: nothing of the old track lingers.
	_midi_player.stop()
	_midi_player.play()
	_fill_midi()
	return true


func _stop_midi() -> void:
	if _midi_song:
		_midi.stop()
		_midi_player.stop()
	_midi_song = false


func _fill_midi() -> void:
	if not _midi_player.playing or _midi_player.stream_paused:
		return
	var pb := _midi_player.get_stream_playback() as AudioStreamGeneratorPlayback
	if pb == null:
		return
	var frames := pb.get_frames_available()
	if frames > 0:
		pb.push_buffer(_midi.render(frames))


## C gain (loudness.c:init_audio): volumeFactorTable[v] =
## 10^((255 - v) × -30 / (20 × 255)), × 2 for the OPL; 0 = mute.
static func volume_to_db(volume: int) -> float:
	if volume <= 0:
		return -80.0
	return -30.0 * float(255 - volume) / 255.0 + 20.0 * log(2.0) / log(10.0)


func _apply_volume() -> void:
	var db := volume_to_db(music_volume)
	if _fade_elapsed >= 0.0:
		db = -80.0 if _fade_elapsed >= FADE_SONG_SECONDS else db - FADE_SONG_DB_PER_S * _fade_elapsed
	_player.volume_db = -80.0 if muted else maxf(db, -80.0)
	# Same table, without the doubling reserved for the OPL.
	_midi_player.volume_db = -80.0 if muted else maxf(db - OPL_GAIN_DB, -80.0)


func _log(msg: String) -> void:
	if log_enabled:
		print("music : t=%d ms %s" % [Time.get_ticks_msec(), msg])
