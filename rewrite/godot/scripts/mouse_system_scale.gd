## Windows pointer acceleration applied to relative motion, like the
## C version: keyboard.c enables SDL_HINT_MOUSE_RELATIVE_SYSTEM_SCALE, and SDL
## (SDL_windowsmouse.c:WIN_UpdateMouseSystemScale, SDL_mouse.c:GetScaledMouseDeltas)
## multiplies each raw relative-mode movement by Windows's "pointer speed"
## setting, with the "Enhance pointer precision" curve
## when it is checked. Godot, with a captured pointer, reads the raw mouse counts
## without this gain. Outside Windows SDL applies nothing: neither do we.
class_name MouseSystemScale
extends RefCounted

## SDL_windowsmouse.c:WIN_SetLinearMouseScale, index = speed 1..20 (SPI_GETMOUSESPEED).
const LINEAR_SCALES := [
	0.0,
	1.0 / 32.0,
	1.0 / 16.0,
	1.0 / 8.0,
	2.0 / 8.0,
	3.0 / 8.0,
	4.0 / 8.0,
	5.0 / 8.0,
	6.0 / 8.0,
	7.0 / 8.0,
	1.0,
	1.25,
	1.5,
	1.75,
	2.0,
	2.25,
	2.5,
	2.75,
	3.0,
	3.25,
	3.5
]
## WIN_SetEnhancedMouseScale: "dpi = 96", "display_factor = 3.5 * (150 / dpi)".
const DISPLAY_FACTOR := 3.5 * (150.0 / 96.0)

## SDL_mouse.c:system_scale_values: one value (fixed gain) or increasing
## {speed, gain} pairs. Empty: no gain (outside Windows, unreadable setting).
static var values := PackedFloat32Array()
## SDL_Mouse.scale_accum_x / scale_accum_y.
static var _accum := Vector2.ZERO
static var _loaded := false


## Reads the Windows setting once (HKCU\Control Panel\Mouse, the values
## returned by SPI_GETMOUSESPEED and SPI_GETMOUSE).
static func ensure_loaded() -> void:
	if _loaded:
		return
	_loaded = true
	if OS.get_name() != "Windows":
		return
	var out := []
	if OS.execute("reg", ["query", "HKCU\\Control Panel\\Mouse"], out) != 0 or out.is_empty():
		push_warning("Mouse: Windows setting unreadable, no gain applied")
		return
	set_from_registry(parse_registry(String(out[0])))
	print("Mouse: Windows gain for relative mode %s" % [values])


## Values from "reg query": name -> text (REG_SZ) or bytes in hexadecimal (REG_BINARY).
static func parse_registry(text: String) -> Dictionary:
	var d := {}
	for line in text.split("\n"):
		var parts := line.strip_edges().split(" ", false)
		if parts.size() >= 3 and parts[1].begins_with("REG_"):
			d[parts[0]] = parts[2]
	return d


## WIN_UpdateMouseSystemScale: enhanced curve if MouseSpeed (params[2] of
## SPI_GETMOUSE) is non-zero, otherwise linear gain of speed 1..20.
static func set_from_registry(reg: Dictionary) -> void:
	values = PackedFloat32Array()
	_accum = Vector2.ZERO
	var speed := int(reg.get("MouseSensitivity", "10"))
	if int(reg.get("MouseSpeed", "0")) != 0:
		var xs := _five_fixed_point(String(reg.get("SmoothMouseXCurve", "")))
		var ys := _five_fixed_point(String(reg.get("SmoothMouseYCurve", "")))
		if xs.size() != 5 or ys.size() != 5:
			return
		var scale := float(speed) / 10.0
		for i in 5:
			var gain := (ys[i] / xs[i]) * scale if xs[i] > 0.0 else 0.0
			values.append(xs[i])
			values.append(gain / DISPLAY_FACTOR)
	elif speed > 0 and speed < LINEAR_SCALES.size():
		values.append(LINEAR_SCALES[speed])


## SDL_windowsmouse.c:LoadFiveFixedPointFloats: five 16.16 numbers on 8 bytes.
static func _five_fixed_point(hex: String) -> PackedFloat32Array:
	var b := hex.hex_decode()
	var out := PackedFloat32Array()
	if b.size() < 40:
		return out
	for i in 5:
		var o := i * 8
		var fraction := float((b[o + 1] << 8) | b[o]) / 65535.0
		out.append(float((b[o + 3] << 8) | b[o + 2]) + fraction)
	return out


## SDL_mouse.c:CalculateSystemScale.
static func _system_scale(d: Vector2i) -> float:
	var n := values.size()
	if n == 1:
		return values[0]
	var speed := sqrt(float(d.x * d.x + d.y * d.y))
	var i := 0
	while i < n - 2:
		if speed < values[i + 2]:
			break
		i += 2
	if i == n - 2:
		return values[n - 1]
	if speed <= values[i]:
		return values[i + 1]
	var coef := (speed - values[i]) / (values[i + 2] - values[i])
	return values[i + 1] + coef * (values[i + 3] - values[i + 1])


## SDL_mouse.c:GetScaledMouseDelta, one axis.
static func _scaled_axis(scale: float, value: int, axis: int) -> int:
	if value == 0 or scale == 1.0:
		return value
	if (value > 0) != (_accum[axis] > 0.0):
		_accum[axis] = 0.0
	_accum[axis] += scale * value
	var out := int(floor(_accum[axis])) if _accum[axis] >= 0.0 else int(ceil(_accum[axis]))
	_accum[axis] -= out
	return out


## Raw movement of ONE event (window pixels) after the Windows gain.
static func apply(d: Vector2i) -> Vector2i:
	if values.is_empty():
		return d
	var scale := _system_scale(d)
	return Vector2i(_scaled_axis(scale, d.x, 0), _scaled_axis(scale, d.y, 1))
