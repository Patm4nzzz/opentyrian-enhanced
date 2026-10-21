# GdUnit4: installing a mod from a .zip (Mods menu, Mods.install_zip).
extends GdUnitTestSuite

const ModsScript := preload("res://scripts/mods.gd")
const FOLDER := "zip_install_test_mod"

var _tmp: String


func before_test() -> void:
	_tmp = ProjectSettings.globalize_path("user://zip_install_test")
	DirAccess.make_dir_recursive_absolute(_tmp)


func after_test() -> void:
	ModsScript._remove_tree(_tmp)
	var installed := ProjectSettings.globalize_path(ModsScript.USER_MODS).path_join(FOLDER)
	if DirAccess.dir_exists_absolute(installed):
		ModsScript._remove_tree(installed)
		Mods.reload()


## Writes an archive `name` containing `entries` (path -> text).
func _zip(name: String, entries: Dictionary) -> String:
	var path := _tmp.path_join(name)
	var z := ZIPPacker.new()
	assert_int(z.open(path)).is_equal(OK)
	for e in entries:
		z.start_file(e)
		z.write_file(String(entries[e]).to_utf8_buffer())
		z.close_file()
	z.close()
	return path


func _mod_json() -> String:
	return JSON.stringify(
		{
			"name": "Zip install test",
			"version": "1.0",
			"author": "test",
			"description": "installed from a zip",
			"game_version": "2.0"
		}
	)


func test_valid_zip_is_installed_and_listed() -> void:
	var zip := _zip("pack.zip", {FOLDER + "/mod.json": _mod_json(), FOLDER + "/README.txt": "x"})
	var dest := ProjectSettings.globalize_path(ModsScript.USER_MODS)
	var r: Dictionary = ModsScript.install_zip(zip, dest)
	assert_str(r["error"]).is_empty()
	assert_str(r["folder"]).is_equal(FOLDER)
	assert_bool(FileAccess.file_exists(dest.path_join(FOLDER).path_join("mod.json"))).is_true()
	assert_bool(FileAccess.file_exists(dest.path_join(FOLDER).path_join("README.txt"))).is_true()
	assert_bool(Mods.available()).is_true()
	Mods.reload()
	var folders := Mods.list().map(func(m: Dictionary) -> String: return m["folder"])
	assert_array(folders).contains([FOLDER])
	# Same mod a second time: refused, the installed one stays.
	assert_str(ModsScript.install_zip(zip, dest)["error"]).is_equal("gd_mods_zip_exists")


func test_mod_json_at_root_takes_the_zip_name() -> void:
	var zip := _zip("rootmod.zip", {"mod.json": _mod_json()})
	var r: Dictionary = ModsScript.install_zip(zip, _tmp.path_join("out"))
	assert_str(r["error"]).is_empty()
	assert_str(r["folder"]).is_equal("rootmod")
	assert_bool(FileAccess.file_exists(_tmp.path_join("out/rootmod/mod.json"))).is_true()


func test_zip_with_escaping_path_is_refused() -> void:
	for bad in ["evil/../../escape.txt", "/abs/mod.json", "C:/x/mod.json", "evil\\..\\x.txt"]:
		var zip := _zip("bad.zip", {"evil/mod.json": _mod_json(), bad: "x"})
		var r: Dictionary = ModsScript.install_zip(zip, _tmp.path_join("out"))
		assert_str(r["error"]).override_failure_message(bad).is_equal("gd_mods_zip_unsafe")
		assert_bool(DirAccess.dir_exists_absolute(_tmp.path_join("out/evil"))).is_false()
	assert_bool(FileAccess.file_exists(_tmp.path_join("escape.txt"))).is_false()


func test_zip_without_mod_json_is_refused() -> void:
	var zip := _zip("nojson.zip", {"a/readme.txt": "x"})
	assert_str(ModsScript.install_zip(zip, _tmp.path_join("out"))["error"]).is_equal("gd_mods_zip_no_json")
	# Two root folders: we do not guess which one is the mod.
	zip = _zip("two.zip", {"a/mod.json": _mod_json(), "b/mod.json": _mod_json()})
	assert_str(ModsScript.install_zip(zip, _tmp.path_join("out"))["error"]).is_equal("gd_mods_zip_no_json")
	assert_bool(DirAccess.dir_exists_absolute(_tmp.path_join("out"))).is_false()


func test_unreadable_zip_is_refused() -> void:
	var path := _tmp.path_join("broken.zip")
	var f := FileAccess.open(path, FileAccess.WRITE)
	f.store_string("not a zip")
	f.close()
	assert_str(ModsScript.install_zip(path, _tmp.path_join("out"))["error"]).is_equal("gd_mods_zip_unreadable")
