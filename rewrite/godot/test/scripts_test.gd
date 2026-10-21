# Loading (parsing) of all the game's GDScript scripts: fails if one does not
# compile. Taken from tools/check_scripts.gd: one check per script.
extends GdUnitTestSuite

const DIRS := ["res://scripts", "res://effects", "res://tools"]


func test_every_script_compiles() -> void:
	var count := 0
	for dir in DIRS:
		for f in DirAccess.get_files_at(dir):
			if not f.ends_with(".gd"):
				continue
			count += 1
			var s: GDScript = load(dir + "/" + f)
			(
				assert_bool(s != null and s.can_instantiate())
				. override_failure_message("does not compile: %s/%s" % [dir, f])
				. is_true()
			)
	# Safeguard: an empty or misnamed folder must not give a hollow green.
	assert_int(count).is_greater(30)
