## Root of a generated level scene (res://levels/epN_NN.tscn):
## three TileMapLayers (Layer1..Layer3) + the events resource.
@tool
class_name LevelMap
extends Node2D

## Row of layer 1 visible at the top of the screen at the start
## (tyrian2.c: mapY = 300 - 8); layer 1 advances one 28 px row
## per 28 curLoc units at normal speed (backMove = 1).
const START_ROW := 300 - 8
const ROW_HEIGHT := 28

@export var episode := 1
@export var lvl_file_num := 1
@export var tileset_name := ""
@export var events: LevelEventResource


func layer(idx: int) -> TileMapLayer:
	return get_node_or_null(LevelMapBuilder.LAYER_NAMES[idx]) as TileMapLayer


## Approximate curLoc so that row `row` of layer 1 is at the top of
## the screen (default scroll speed, no map stop).
static func cur_loc_for_row(row: int) -> int:
	return maxi(0, (START_ROW - row) * ROW_HEIGHT)


## "Play from here" launch request (ADR 0126): written by the editor
## plugin, where the Game autoload does not exist, then read (and deleted) by
## Game at game start, which passes it to Game.start_level_at.
const START_REQUEST_FILE := "user://play_from_here.json"


static func write_start_request(episode: int, lvl_file_num: int, cur_loc: int) -> void:
	var f := FileAccess.open(START_REQUEST_FILE, FileAccess.WRITE)
	f.store_string(JSON.stringify({"episode": episode, "lvl_file_num": lvl_file_num, "cur_loc": cur_loc}))
	f.close()


## Returns {episode, lvl_file_num, cur_loc} and deletes the file; {} if there
## is no request (or if it is unreadable).
static func take_start_request() -> Dictionary:
	if not FileAccess.file_exists(START_REQUEST_FILE):
		return {}
	var req: Variant = JSON.parse_string(FileAccess.get_file_as_string(START_REQUEST_FILE))
	DirAccess.remove_absolute(ProjectSettings.globalize_path(START_REQUEST_FILE))
	return req if req is Dictionary else {}
