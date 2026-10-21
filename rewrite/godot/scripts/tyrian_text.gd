## Text with the bitmap fonts of `tyrian.shp` — stable API shared by
## all interface scenes (title, menus, shop, debriefing...).
##
## Two uses:
## 1. Exact rendering (recommended, ADR 0020): static functions that draw
##    in a `TyrianScreen` (the C's 8-bit framebuffer), same names and same
##    parameters as the C code:
##      TyrianText.draw_font_hv(screen, x, y, text, font, align, hue, value)
##      TyrianText.draw_font_hv_shadow(screen, x, y, text, font, align, hue, value, black, dist)
##      TyrianText.draw_font_hv_full_shadow(...), draw_font_dark(...), draw_font_hv_blend(...)
##      TyrianText.text_shade(screen, x, y, text, colorbank, brightness, shadetype)   # JE_textShade
##      TyrianText.out_text(...)          # JE_outText
##      TyrianText.out_text_adjust(...)   # JE_outTextAdjust
##      TyrianText.out_text_and_darken(...), out_text_modify(...), dstring(...)
##      TyrianText.width(text, font)      # JE_textWidth
##      TyrianText.font_center(text, font)  # JE_fontCenter
##      TyrianText.L(key)                 # strings of data/locales (locale.c:L)
##    `surf` optional as the last parameter (TyrianScreen.VGA_SCREEN by default).
##    "~" toggles the highlight as in C.
## 2. `TyrianText` node (old RGBA rendering, kept for the level editor
##    and screens not yet ported): approximation, no pixel parity.
@tool
class_name TyrianText
extends Node2D

## Same order as font.h (large_font, normal_font, small_font).
enum FontKind { LARGE, NORMAL, SMALL }
enum Align { LEFT, CENTER, RIGHT }

## fonthand.h: shadow types of JE_textShade.
const PART_SHADE := 0
const FULL_SHADE := 1
const DARKEN := 2
const TRICK := 3

const SHEETS := ["main_00_font", "main_01_small_font", "main_02_tiny_font"]

## Codes 32..126 -> sprite index (fonthand.c:font_ascii), -1 = absent.
const FONT_ASCII := [
	-1,
	26,
	33,
	60,
	61,
	62,
	-1,
	32,
	64,
	65,
	63,
	84,
	29,
	83,
	28,
	80,
	79,
	70,
	71,
	72,
	73,
	74,
	75,
	76,
	77,
	78,
	31,
	30,
	-1,
	85,
	-1,
	27,
	-1,
	0,
	1,
	2,
	3,
	4,
	5,
	6,
	7,
	8,
	9,
	10,
	11,
	12,
	13,
	14,
	15,
	16,
	17,
	18,
	19,
	20,
	21,
	22,
	23,
	24,
	25,
	68,
	82,
	69,
	-1,
	-1,
	-1,
	34,
	35,
	36,
	37,
	38,
	39,
	40,
	41,
	42,
	43,
	44,
	45,
	46,
	47,
	48,
	49,
	50,
	51,
	52,
	53,
	54,
	55,
	56,
	57,
	58,
	59,
	66,
	81,
	67,
	-1,
]

static var _locale := {}
static var _locale_lang := ""
## Code of the current language (locale.c:language_codes[current_language]).
static var language := "en"

# --- Exact rendering API (TyrianScreen) ---------------------------------------


## `fonthand.c:JE_textWidth`.
static func width(text: String, font: int) -> int:
	if not ClassDB.class_exists("TyrianVga"):
		return measure(text, font)
	ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
	return ClassDB.class_call_static("TyrianVga", "text_width", text, font)


## `fonthand.c:JE_fontCenter`.
static func font_center(text: String, font: int) -> int:
	return 160 - width(text, font) / 2


static func draw_font_hv(
	screen: TyrianScreen, x: int, y: int, text: String, font: int, align: int, hue: int, value: int, surf := 0
) -> void:
	screen.vga.draw_font_hv(surf, x, y, text, font, align, hue, value)


static func draw_font_hv_blend(
	screen: TyrianScreen, x: int, y: int, text: String, font: int, align: int, hue: int, value: int, surf := 0
) -> void:
	screen.vga.draw_font_hv_blend(surf, x, y, text, font, align, hue, value)


static func draw_font_dark(
	screen: TyrianScreen, x: int, y: int, text: String, font: int, align: int, black: bool, surf := 0
) -> void:
	screen.vga.draw_font_dark(surf, x, y, text, font, align, black)


static func draw_font_hv_shadow(
	screen: TyrianScreen,
	x: int,
	y: int,
	text: String,
	font: int,
	align: int,
	hue: int,
	value: int,
	black: bool,
	dist: int,
	surf := 0
) -> void:
	screen.vga.draw_font_hv_shadow(surf, x, y, text, font, align, hue, value, black, dist)


static func draw_font_hv_full_shadow(
	screen: TyrianScreen,
	x: int,
	y: int,
	text: String,
	font: int,
	align: int,
	hue: int,
	value: int,
	black: bool,
	dist: int,
	surf := 0
) -> void:
	screen.vga.draw_font_hv_full_shadow(surf, x, y, text, font, align, hue, value, black, dist)


static func text_shade(
	screen: TyrianScreen, x: int, y: int, text: String, colorbank: int, brightness: int, shadetype: int, surf := 0
) -> void:
	screen.vga.text_shade(surf, x, y, text, colorbank, brightness, shadetype)


static func out_text(
	screen: TyrianScreen, x: int, y: int, text: String, colorbank: int, brightness: int, surf := 0
) -> void:
	screen.vga.out_text(surf, x, y, text, colorbank, brightness)


static func out_text_adjust(
	screen: TyrianScreen, x: int, y: int, text: String, filter: int, brightness: int, font: int, shadow: bool, surf := 0
) -> void:
	screen.vga.out_text_adjust(surf, x, y, text, filter, brightness, font, shadow)


static func out_text_and_darken(
	screen: TyrianScreen, x: int, y: int, text: String, colorbank: int, brightness: int, font: int, surf := 0
) -> void:
	screen.vga.out_text_and_darken(surf, x, y, text, colorbank, brightness, font)


static func out_text_modify(
	screen: TyrianScreen, x: int, y: int, text: String, filter: int, brightness: int, font: int, surf := 0
) -> void:
	screen.vga.out_text_modify(surf, x, y, text, filter, brightness, font)


static func dstring(screen: TyrianScreen, x: int, y: int, text: String, font: int, surf := 0) -> void:
	screen.vga.dstring(surf, x, y, text, font)


## helptext.c:verticalHeight (7 by default; the shop sets it to 9).
static var vertical_height := 7


## `helptext.c:JE_helpBox`: text split at spaces into lines of about
## `boxwidth` characters, same 1-based arithmetic as the C code (a too-long word
## emitted alone on its line). Color/brightness/shadow = helpBoxColor,
## helpBoxBrightness, helpBoxShadeType (12, 1, FULL_SHADE by default).
static func help_box(
	screen: TyrianScreen,
	x: int,
	y: int,
	message: String,
	boxwidth: int,
	color := 12,
	brightness := 1,
	shadetype := FULL_SHADE,
	surf := 0
) -> void:
	var n := message.length()
	if n == 0:
		return
	var pos := 1
	var endpos := 0
	var startpos := 0
	var endstring := false
	while true:
		startpos = endpos + 1
		while true:
			endpos = pos
			while true:
				pos += 1
				if pos >= n:
					endstring = true
					if pos - startpos < boxwidth:
						endpos = pos + 1
				if endstring or message[pos - 1] == " ":
					break
			if pos - startpos > boxwidth or endstring:
				break
		if endpos <= startpos:
			endpos = pos + 1 if endstring else pos
		text_shade(screen, x, y, message.substr(startpos - 1, endpos - startpos), color, brightness, shadetype, surf)
		y += vertical_height
		if endstring:
			break
	if endpos != pos + 1:
		text_shade(screen, x, y, message.substr(endpos), color, brightness, shadetype, surf)


## `locale.c:L`: string from `data/locales/locale_<language>.txt` (key=value),
## the key itself if absent (same fallback as the C code). Without `lang`, the current
## language (`language`, locale.c:current_language, changed by Setup > Gameplay).
static func L(key: String, lang := "") -> String:
	if lang == "":
		lang = language
	if _locale_lang != lang:
		_locale = {}
		_locale_lang = lang
		var f := FileAccess.open(Game.data_dir.path_join("locales/locale_%s.txt" % lang), FileAccess.READ)
		if f != null:
			while not f.eof_reached():
				var line := f.get_line()
				if line.begins_with("#") or not line.contains("="):
					continue
				var kv := line.split("=", true, 1)
				# Non-breaking space -> space, like locale.c:utf8_to_cp437 at
				# load time (same line breaks as the C code).
				_locale[kv[0].strip_edges()] = kv[1].c_unescape().replace("\u00a0", " ")
	return _locale.get(key, key)


# --- TyrianText node (approximate RGBA rendering) ------------------------------

@export var text := "":
	set(v):
		text = v
		queue_redraw()
@export var font := FontKind.LARGE:
	set(v):
		font = v
		queue_redraw()
@export var align := Align.LEFT:
	set(v):
		align = v
		queue_redraw()
## One-pixel drop shadow (JE_textShade FULL_SHADE).
@export var shadow := true:
	set(v):
		shadow = v
		queue_redraw()
## Dark outline (title_screen.c: 4 copies offset diagonally, brightness -10).
@export var outline := false:
	set(v):
		outline = v
		queue_redraw()
@export var color := Color.WHITE:
	set(v):
		color = v
		queue_redraw()


static func sprite_id(code: int) -> int:
	if code < 32 or code > 126:
		return -1
	return FONT_ASCII[code - 32]


## Glyph index of each character of `s` in font `which`, after
## the C conversion (locale.c:utf8_to_cp437: "à" -> "a", "ß" -> "ss",
## typographic punctuation to ASCII); space = -2, no glyph = -1.
static func glyphs(s: String, which: int) -> PackedInt32Array:
	if ClassDB.class_exists("TyrianVga"):
		ClassDB.class_call_static("TyrianVga", "load_assets", Game.data_dir)
		return ClassDB.class_call_static("TyrianVga", "text_glyphs", s, which)
	var out := PackedInt32Array()
	for ch in s:
		var code := ch.unicode_at(0)
		out.append(-2 if code == 32 else sprite_id(code))
	return out


static func measure(s: String, which: int) -> int:
	var sheet := TyrianAssets.sheet(SHEETS[which])
	if sheet == null:
		return 0
	var w := 0
	for id in glyphs(s, which):
		if id == -2:
			w += 6
		elif id >= 0 and sheet.has(id):
			w += sheet.width(id) + 1
	return w


func _draw() -> void:
	var sheet := TyrianAssets.sheet(SHEETS[font])
	if sheet == null:
		return
	var x := 0.0
	match align:
		Align.CENTER:
			x = -measure(text, font) / 2.0
		Align.RIGHT:
			x = -measure(text, font)
	for id in glyphs(text, font):
		if id == -2:
			x += 6
			continue
		if id < 0 or not sheet.has(id):
			continue
		if outline:
			for d in [Vector2(-1, -1), Vector2(1, 1), Vector2(1, -1), Vector2(-1, 1)]:
				sheet.draw(self, id, x + d.x, d.y, Color(0.08, 0.06, 0.1))
		elif shadow:
			sheet.draw(self, id, x + 1, 1, Color(0, 0, 0, 0.8))
		sheet.draw(self, id, x, 0, color)
		x += sheet.width(id) + 1
