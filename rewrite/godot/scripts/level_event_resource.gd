## Level events (level_events.c, `eventRec`), editable in
## the Godot inspector. One entry = a dictionary with the EVENT_KEYS keys.
@tool
class_name LevelEventResource
extends Resource

const EVENT_KEYS := [
	"eventtime", "eventtype", "eventdat", "eventdat2", "eventdat3", "eventdat4", "eventdat5", "eventdat6"
]

@export var episode := 1
@export var lvl_file_num := 1
## mapX, mapX2, mapX3 from the level header.
@export var map_x := PackedInt32Array([1, 1, 1])
## Random enemies of the level (`levelEnemy`).
@export var random_enemies := PackedInt32Array()
@export var events: Array[Dictionary] = []
