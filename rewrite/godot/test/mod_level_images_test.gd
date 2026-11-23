## Mod images in a level (enemy_render.c:blit_enemy, enemy_overrides): a PNG
## named after an original enemy replaces that enemy's image in the 8-bit
## composition of the play area (TyrianSim.playfield_background), mapped to the
## palette of the level. The same level without the mod draws the original
## image (docs/divergences.md, entry 69).
extends GdUnitTestSuite

const TEMP_MOD := "qa_level_images"
## Ground enemies of TYRIAN (episode 1, level file 9), bank 1.
const ENEMIES := [
	"tech_corridor_35",
	"tech_corridor_32",
	"tech_corridor_33",
	"tech_corridor_21",
	"tech_corridor_25",
	"tech_corridor_26",
	"tech_corridor_38",
	"tech_corridor_23",
	"tech_corridor_17",
	"tech_corridor_18",
]
const PLAY_AREA_X := 24
const TICKS := 250


func before() -> void:
	EndlessSession.stop()
	Game.episode = 1


func after() -> void:
	_remove_temp_mod()


## Level palette (dashboard picture 3) and the color of entry `index`.
func _palette() -> PackedByteArray:
	var vga: RefCounted = ClassDB.instantiate("TyrianVga")
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	assert_bool(vga.load_pic(0, 3, true)).is_true()
	return vga.palette_bytes()


func _mod_dir() -> String:
	return ProjectSettings.globalize_path(Mods.USER_MODS).path_join(TEMP_MOD)


## One 12x14 PNG of a single color per enemy name.
func _write_temp_mod(color: Color) -> void:
	var dir := _mod_dir()
	DirAccess.make_dir_recursive_absolute(dir.path_join("sprites/enemies"))
	var img := Image.create(12, 14, false, Image.FORMAT_RGBA8)
	img.fill(color)
	for enemy: String in ENEMIES:
		img.save_png(dir.path_join("sprites/enemies/%s.png" % enemy))
	var f := FileAccess.open(dir.path_join("mod.json"), FileAccess.WRITE)
	f.store_string('{"format_version": 1, "name": "QA level images"}')
	f.close()


func _remove_temp_mod() -> void:
	var dir := _mod_dir()
	Mods.set_enabled(TEMP_MOD, false)
	for enemy: String in ENEMIES:
		DirAccess.remove_absolute(dir.path_join("sprites/enemies/%s.png" % enemy))
	for rel in ["mod.json", "sprites/enemies", "sprites"]:
		DirAccess.remove_absolute(dir.path_join(rel))
	DirAccess.remove_absolute(dir)
	Mods.reload()


## TYRIAN played TICKS ticks with the fire button held, mods as enabled.
func _play() -> Node:
	var sim: Node = auto_free(ClassDB.instantiate("TyrianSim"))
	Mods.attach(sim)
	sim.set_loadout(PackedInt32Array([1, 1, 1, 0, 1, 4, 2, 0, 0, 0, 0]))
	assert_bool(sim.load_level(Game.data_dir, 1, 9, 2, 7)).is_true()
	for i in TICKS:
		sim.step(16, 0)
	return sim


func test_mod_enemy_image_is_drawn_in_the_play_area() -> void:
	var pal := _palette()
	# A color of the level palette: the mapping gives it back exactly.
	var entry := 0
	for i in range(32, 256):
		if pal[i * 3] > 150 and pal[i * 3 + 1] < 90 and pal[i * 3 + 2] < 90:
			entry = i
			break
	assert_int(entry).override_failure_message("no red entry in the level palette").is_greater(0)
	var color := Color8(pal[entry * 3], pal[entry * 3 + 1], pal[entry * 3 + 2])
	_write_temp_mod(color)
	Mods.set_enabled(TEMP_MOD, true)
	Mods.reload()
	assert_bool(Mods.has_textures("enemies")).override_failure_message("enemy PNGs not seen").is_true()
	var sim := _play()
	var with_mod: Image = sim.playfield_background(Game.data_dir, pal)
	# Pieces drawn normally (mode 0) whose image a mod replaces, fully on screen.
	var spots: Array[Vector2i] = []
	var recs: PackedInt32Array = sim.hd_enemy_sprites()
	for i in range(0, recs.size() - 6, 7):
		var at := Vector2i(recs[i + 2] - PLAY_AREA_X + 5, recs[i + 3] + 6)
		var on_screen := at.x >= 6 and at.x < 250 and at.y >= 8 and at.y < 170
		if recs[i + 5] == 0 and on_screen and Mods.enemy_texture(recs[i + 1], recs[i + 4]) != null:
			spots.append(at)
	assert_array(spots).override_failure_message("no replaced enemy on screen at tick %d" % TICKS).is_not_empty()
	var shown := 0
	for at in spots:
		if with_mod.get_pixelv(at).is_equal_approx(color):
			shown += 1
	assert_int(shown).override_failure_message("no mod enemy image in the composed play area").is_greater(0)
	# Same game without the mod: the original images, not that color.
	Mods.set_enabled(TEMP_MOD, false)
	Mods.reload()
	var original: Image = _play().playfield_background(Game.data_dir, pal)
	var still := 0
	for at in spots:
		if original.get_pixelv(at).is_equal_approx(color):
			still += 1
	assert_int(still).override_failure_message("the image stayed without the mod").is_equal(0)


## Sprite nodes of the level (HD sprite mode, and the enemy layers the 8-bit
## picture leaves to the nodes): a mod image goes on the pooled sprite whole,
## at the corner of the sheet cell it replaces; an image larger than one cell
## (12x14) stands for the whole 2x2 enemy, whose other pieces are not drawn.
func test_sprite_nodes_take_the_mod_image_and_skip_covered_pieces() -> void:
	_write_temp_mod(Color(1, 0, 0, 1))
	# One enemy gets a 24x28 image: it covers the cells +1, +19 and +20.
	var big := Image.create(24, 28, false, Image.FORMAT_RGBA8)
	big.fill(Color(0, 1, 0, 1))
	big.save_png(_mod_dir().path_join("sprites/enemies/%s.png" % ENEMIES[0]))
	Mods.set_enabled(TEMP_MOD, true)
	Mods.reload()
	var big_at := Vector2i.ZERO
	var small_at := Vector2i.ZERO
	for s in Mods.api.sprites(1):
		if String(s["table"]) != "enemies":
			continue
		var at := Vector2i(int(s["sheet"]), int(s["index"]))
		if String(s["path"]).get_file().get_basename() == ENEMIES[0]:
			big_at = at
		elif small_at == Vector2i.ZERO:
			small_at = at
	assert_int(big_at.x).override_failure_message("sheet byte of the enemy bank missing").is_greater(0)
	var level: Node = auto_free(preload("res://scripts/level.gd").new())
	var tex := Mods.enemy_texture(big_at.x, big_at.y)
	assert_object(tex).is_not_null()
	assert_that(Vector2i(tex.get_size())).is_equal(Vector2i(24, 28))
	for piece in [1, 19, 20]:
		(
			assert_bool(level._mod_image_covers(big_at.x, big_at.y + piece))
			. override_failure_message("piece +%d of the 2x2 enemy still drawn" % piece)
			. is_true()
		)
	# A one-cell image covers nothing but its own cell.
	if Mods.enemy_texture(small_at.x, small_at.y + 1) == null and small_at.y + 1 != big_at.y + 20:
		assert_bool(level._mod_image_covers(small_at.x, small_at.y + 20)).is_false()
	var sprite: Sprite2D = auto_free(Sprite2D.new())
	sprite.region_enabled = true
	level._apply_mod_image(sprite, tex, 100, 50)
	assert_object(sprite.texture).is_same(tex)
	assert_bool(sprite.region_enabled).is_false()
	assert_that(sprite.position).is_equal(Vector2(100 - PLAY_AREA_X, 50))
	assert_bool(sprite.visible).is_true()
