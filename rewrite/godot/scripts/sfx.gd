## Sound effects (autoload `Sfx`): WAVs from tyrian.snd (sfx/<n>.wav,
## 1-based numbering of JE_playSampleNum, see src/sndmast.h).
##
## Eight channels like the C mixer (loudness.c:CHANNEL_COUNT): each has its
## own player and its "Sfx<n>" bus, carrying a TyrianStereoGain (extension)
## that applies the channel's left / right gain (loudness.c: chanGainL /
## chanGainR derived from channelPan). Level sounds come from the simulation
## queue (TyrianSim.take_sounds: channel, sample, soundQueueX,
## soundQueueDmg) and go through play_queued, which takes over
## loudness.c:JE_flushSoundQueue (volume per channel, panning, pitch and
## volume according to damage); menus go through play
## (nortsong.c:JE_playSampleNum, centered).
extends Node

const WEAPON_1 := 1
const EXPLOSION_9 := 9
const SELECT := 8
const ITEM := 18
const SPRING := 16
const CURSOR := 28
const CLICK := 24
## sndmast.h:S_CLINK (volume limit, nortsong.c:JE_changeVolume).
const CLINK := 23

const CHANNELS := 8
## sndmast.h:SAMPLE_COUNT: samples of tyrian.snd.
const SAMPLE_COUNT := 81
## nortsong.c:fxPlayVol (channel levels 0..7, CHANNEL_VOLUME_LEVELS = 8).
const FX_PLAY_VOL := 4
## loudness.c:screen_x_to_pan, center (soundQueueX = 160).
const PAN_CENTER := 128

var enabled := true
## Command-line option -s / --no-sound (opentyr.c: audio_disabled):
## takes precedence over the Setup setting, which does not lift it.
var no_sound: bool = OS.get_cmdline_user_args().has("-s") or OS.get_cmdline_user_args().has("--no-sound")
## Setup > Sound: fxVolume (0..255, 191 by default) and pitch variation
## (nortsong.c:sfx_pitch_variation, ±5 % per effect).
var volume := 191:
	set(v):
		volume = clampi(v, 0, 255)
		for i in _players.size():
			_players[i].volume_db = _volume_db() + _channel_db[i]
var pitch_variation := true
## Setup > Sound > Stereo (loudness.c:stereo_enabled): effects placed according to their
## on-screen x; off, everything is centered (channelPan = 128).
var stereo := true
## Setup > Sound > SFX Impact Scaling (nortsong.c:sfx_impact_scaling): big
## enemies and bosses explode deeper and louder (soundQueueDmg).
var impact_scaling := true
## Tests: also plays without a display (the dummy audio driver mixes in headless).
var play_in_headless := false
## True after release_streams: no effect is loaded any more before shutdown.
var _released := false
var _players: Array[AudioStreamPlayer] = []
## TyrianStereoGain of each channel (null without the extension).
var _pans: Array = []
## Channel-specific gain (volume level of the sound being played), in dB.
var _channel_db: Array[float] = []
var _streams := {}


func _ready() -> void:
	for i in CHANNELS:
		var p := AudioStreamPlayer.new()
		p.name = "Channel%d" % i
		p.bus = _ensure_bus(i)
		add_child(p)
		_players.append(p)
		_channel_db.append(0.0)
		p.volume_db = _volume_db()
	_preload_streams()


## Loads all samples at launch. The C code has them all in memory;
## loading them at the first sound blocked the main loop: several
## seconds at each new effect in the Windows exe and under WSL (the first
## SPRING of Ship Specs froze the screen and swallowed inputs).
func _preload_streams() -> void:
	for number in range(1, SAMPLE_COUNT + 1):
		_stream(number)


## Christmas mode enabled or disabled: the voices (32 to 40) are re-read (voicesc.snd).
func reload_voices() -> void:
	for number in range(Christmas.VOICE_FIRST, Christmas.VOICE_LAST + 1):
		_streams.erase(number)
		_stream(number)


## "Sfx<i>" bus sent to Master, with its TyrianStereoGain.
func _ensure_bus(i: int) -> StringName:
	var bus_name := StringName("Sfx%d" % i)
	var idx := AudioServer.get_bus_index(bus_name)
	if idx < 0:
		AudioServer.add_bus()
		idx = AudioServer.bus_count - 1
		AudioServer.set_bus_name(idx, bus_name)
		AudioServer.set_bus_send(idx, &"Master")
	var pan: AudioEffect = null
	if ClassDB.class_exists(&"TyrianStereoGain"):
		pan = ClassDB.instantiate(&"TyrianStereoGain")
		AudioServer.add_bus_effect(idx, pan, 0)
	_pans.append(pan)
	return bus_name


func _stream(number: int) -> AudioStream:
	if _streams.has(number):
		return _streams[number]
	var path := TyrianAssets.IMPORTED.path_join(Christmas.sfx_path(number))
	var stream: AudioStream = null
	if ResourceLoader.exists(path):
		stream = load(path)
	elif FileAccess.file_exists(path):
		stream = AudioStreamWAV.load_from_file(ProjectSettings.globalize_path(path))
	_streams[number] = stream
	return stream


## Program shutdown: releases the channel streams (see Music.release_streams).
## C gain (loudness.c): volumeFactorTable[v] × (level+1)/8; here the table
## alone, plus the menu level (fxPlayVol) since level_db is relative to it.
## At 191: −11.6 dB (0.42 × 5/8). Music.volume_to_db carries a +6.02 dB
## (OPL doubling) that is removed to get back the C table.
func _volume_db() -> float:
	if volume <= 0:
		return -80.0
	return Music.volume_to_db(volume) - 20.0 * log(2.0) / log(10.0) + linear_to_db(float(FX_PLAY_VOL + 1) / 8.0)


## loudness.c:stop_all_samples (called on entering the title, title_screen.c:462):
## cuts every voice/effect still playing from the level that was left.
func stop_all() -> void:
	for p in _players:
		p.stop()


func release_streams() -> void:
	_released = true
	for p in _players:
		p.stop()
		p.stream = null
	_streams.clear()


## nortsong.c:random_pitch_step(5): integer delta in [-5, +5] %.
func _random_pitch() -> float:
	return 1.0 + (randi_range(-5, 5) / 100.0 if pitch_variation else 0.0)


## Relative gain of a channel level (loudness.c: sampleVolumeFactors[v] =
## factor × (v + 1) / 8) with respect to fxPlayVol, the menu level.
static func level_db(level: int) -> float:
	return linear_to_db(float(clampi(level, 0, 7) + 1) / float(FX_PLAY_VOL + 1))


## Menu sounds requested since launch, in order: read by the tests
## (one integer per sound, a few bytes per hour of play).
var played: Array = []


## Menus and out-of-queue sounds (nortsong.c:JE_playSampleNum): fxPlayVol, centered,
## always on channel 0 (nortsong.c:287), which replaces the sound in progress. In
## a level, this channel is also the front gun's: the next shot cuts it, as in the
## C code (the achievement sound, cut that way, is no longer played in a level:
## divergence 63).
func play(number: int) -> void:
	played.append(number)
	_start(0, number, FX_PLAY_VOL, PAN_CENTER, _random_pitch())


## loudness.c:JE_flushSoundQueue for a sound from the simulation queue:
## channel 3 at fxPlayVol, sample 15 (lightning) at a quarter, the rest at
## half; panning according to soundQueueX; then, if Impact Scaling, pitch
## 0.80 (big enemy, dmg 2) or 0.65 (boss, dmg ≥ 3) and raised volume.
func play_queued(chan: int, number: int, x: int, dmg: int) -> void:
	var level := FX_PLAY_VOL if chan == 3 else (FX_PLAY_VOL / 4 if number == 15 else FX_PLAY_VOL / 2)
	var pitch := _random_pitch()
	if not impact_scaling:
		dmg = 0
	if dmg > 0:
		# 0xA666 / 0x10000 = 0.65; 0xCCCD / 0x10000 = 0.80.
		pitch *= 0.65 if dmg >= 3 else (0.8 if dmg == 2 else 1.0)
		if dmg >= 2 and level < FX_PLAY_VOL:
			level = FX_PLAY_VOL if dmg >= 3 else (level + FX_PLAY_VOL) / 2
	_start(clampi(chan, 0, CHANNELS - 1), number, level, screen_x_to_pan(x), pitch)


## loudness.h:screen_x_to_pan.
static func screen_x_to_pan(x: int) -> int:
	return clampi(x - 24, 0, 264) * 255 / 264


## loudness.c:multiSamplePlayPanPitch: the sound replaces the channel's;
## channelPan = stereo_enabled ? pan : 128.
func _start(chan: int, number: int, level: int, pan: int, pitch: float) -> void:
	if _released or not enabled or no_sound:
		return
	var s := _stream(number)
	if s == null:
		return
	var p := _players[chan]
	_channel_db[chan] = level_db(level)
	p.volume_db = _volume_db() + _channel_db[chan]
	p.pitch_scale = pitch
	if _pans[chan] != null:
		_pans[chan].set_pan(pan if stereo else PAN_CENTER)
	p.stream = s
	if play_in_headless or DisplayServer.get_name() != "headless":
		p.play()


## (left, right) gains of the channel (tests).
func channel_gains(chan: int) -> Vector2:
	if _pans[chan] == null:
		return Vector2.ONE
	return Vector2(_pans[chan].left_gain(), _pans[chan].right_gain())
