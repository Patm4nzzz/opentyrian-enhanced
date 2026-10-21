## "Instructions" help of the title (mainint.c:JE_helpSystem(1) and
## helpSystemPage), rendered frame by frame on TyrianScreen (ADR 0020).
##
## Topic menu (topicName_1..5) then pages 1..MAX_PAGE of help boxes
## (JE_HBox → help_<n-1>). Mouse: hover and click on topics, right click =
## exit; within a page, left click left / right of center = previous /
## next page (left / right pointers), right click = topics.
extends TyrianScreen

## Starting topic: 1 = topic menu (title), 2 = page 1 of topic 2
## (JE_helpSystem(2), F1 in the shop).
signal closed

const SCENE_TITLE := "res://scenes/title.tscn"

## mainint.c: MAX_PAGE, topicStart.
## The "Keys and mouse" topic (pages 9 to 11) is an addition (divergence no. 57).
const MAX_PAGE := 11
const TOPIC_START := [0, 1, 2, 3, 7, 9, 255]
## topicName: 7 entries (header + 5 topics + exit).
const TOPIC_COUNT := 7
const X_CENTER := 160
const Y_MENU_HEADER := 30
const Y_MENU_ITEMS := 60

## Key pages: HELP_KEYS rows (start, end excluded) of each page.
const KEY_PAGES := {9: [0, 6], 10: [6, 12], 11: [12, 16]}
const KEY_PAGE_Y := 20
const KEY_PAGE_STEP := 28

## helpSystemPage: boxes (y, message number) of each page, width 60.
const PAGE_BOXES := {
	1: [[20, 2], [50, 5], [80, 21], [110, 1], [140, 28]],
	2: [[20, 1], [60, 2], [100, 21], [140, 28]],
	3: [[20, 5], [70, 6], [110, 7]],
	4: [[20, 8], [55, 9], [87, 10], [120, 11], [170, 13]],
	5: [[20, 14], [80, 15], [120, 16]],
	6: [[20, 17], [40, 18], [130, 20]],
	7: [[20, 21], [70, 22], [110, 23], [140, 24]],
	8: [[20, 25], [60, 26], [100, 27], [140, 28], [170, 29]],
}

var selected_index := 0
## 0: topic menu; otherwise displayed page number.
var page := 0
## Current topic (1-based, like the C's `topic`).
var topic := 1
var idle := false
var _busy := true
## Opened by the shop (F1): child of the scene instead of a scene, exit
## emits "closed" instead of returning to the title.
var embedded := false
var start_topic := 1
## The C's "restart": the fade-in only happens once.
var _fade_pending := true
## Topic menu (shared engine, ADR 0121).
var menus := MenuEngine.new(self)


func _ready() -> void:
	_run.call_deferred()


func _run() -> void:
	# `if (restart)`: map music, background = image 2 in VGAScreen2.
	Music.play_song(Music.SONG_MAPVIEW)
	vga.load_pic(VGA_SCREEN2, 2, false)
	if start_topic > 1:
		# JE_helpSystem(topic > 1): helpSystemPage starts from topicStart[topic - 1].
		topic = start_topic
		page = TOPIC_START[topic - 1]
		_draw_page()
		idle = false
		_fade_pending = false
		await fade_palette(10, 0, 255)
		idle = true
		_busy = false
		return
	await _draw_menu()
	_busy = false


func _topic_name(i: int) -> String:
	return TyrianText.L("topicName_%d" % i)


## Body of the JE_helpSystem loop (topic 1: menu): shared engine,
## TOPICS style (ADR 0121).
func _draw_menu() -> void:
	if menus.stack.is_empty():
		var items := []
		for i in TOPIC_COUNT - 1:
			items.append(MenuEngine.item(_topic_name(i + 1), "", MenuEngine.Kind.ACTION))
		await menus.open(_topic_name(0), items, {
			"style": MenuEngine.Style.TOPICS, "move_sfx": Sfx.CURSOR, "redraw_on_other": true,
			"accept": [TyrianControls.SELECT, TyrianControls.ACCEPT],
			"on_activate": _choose_topic,
			"on_back": func() -> void:
				Sfx.play(Sfx.SPRING)
				_leave(),
			"pre_present": func() -> void:
				if _fade_pending:
					_fade_pending = false
					await fade_palette(10, 0, 255),
		})
		return
	menus.current().sel = selected_index
	await menus.draw()


## Body of the helpSystemPage loop. Returns false when the page leaves the
## topic (back to the menu, selection set as on the C's return).
func _draw_page() -> bool:
	if page == 0:
		topic = 1
		return false
	if page > MAX_PAGE:
		topic = TOPIC_COUNT - 1
		return false
	for t in TOPIC_COUNT:
		if TOPIC_START[t] <= page:
			topic = t + 1
		else:
			break
	vga.copy(VGA_SCREEN, VGA_SCREEN2)
	vga.fill_rectangle_xy(VGA_SCREEN, 0, 192, 319, 199, 0)  # fill_rectangle_wh(0, 192, 320, 8)
	TyrianText.draw_font_hv_shadow(self, X_CENTER, 1, _topic_name(topic - 1), NORMAL_FONT, CENTERED, 15, -3, false, 2)
	TyrianText.draw_font_hv(self, 10, 192, "%s %d" % [TyrianText.L("miscText_24"), page - TOPIC_START[topic - 1] + 1],
		SMALL_FONT, LEFT, 13, 5)
	# miscText[25] is overridden by "misc_of" (helptext.c:JE_refreshLocalizedMenuText).
	TyrianText.draw_font_hv(self, 320 - 10, 192,
			"%s %d %s %d" % [TyrianText.L("misc_page"), page, TyrianText.L("misc_of"), MAX_PAGE],
		SMALL_FONT, RIGHT, 13, 5)
	# helpBoxBrightness = 3, verticalHeight = 8 during the boxes.
	TyrianText.vertical_height = 8
	if KEY_PAGES.has(page):
		for i in range(KEY_PAGES[page][0], KEY_PAGES[page][1]):
			var line: Array = TyrianControls.HELP_KEYS[i]
			TyrianText.help_box(self, 10, KEY_PAGE_Y + (i - KEY_PAGES[page][0]) * KEY_PAGE_STEP,
				TyrianControls.help_line(line[0], line[1]), 60, 12, 3)
	else:
		for box in PAGE_BOXES[page]:
			TyrianText.help_box(self, 10, box[0], TyrianText.L("help_%d" % (box[1] - 1)), 60, 12, 3)
	TyrianText.vertical_height = 7
	present()
	idle = true
	return true


func handle_action(key: StringName) -> void:
	if _busy or not idle:
		return
	if page == 0:
		menus.handle_action(key)
		selected_index = menus.selection()
	else:
		_page_key(key)


## Mouse: topics through the shared engine (TOPICS style); pages like
## mainint.c:helpSystemPage (mouseCursor = mouse_x < xCenter ? LEFT : RIGHT,
## left click = page -1 / +1, right click = back to topics). Wheel
## (addition, absent from the C): previous / next topic, previous / next page.
func handle_mouse(event: InputEvent) -> void:
	if _busy or not idle:
		return
	TyrianMouse.note(event)
	if page == 0:
		mouse_pointer = MouseCursor.NORMAL
		menus.handle_mouse(event)
		if page == 0 and not menus.stack.is_empty():
			selected_index = menus.selection()
		return
	mouse_pointer = MouseCursor.LEFT if event.position.x < X_CENTER else MouseCursor.RIGHT
	if TyrianMouse.is_wheel(event):
		return
	if not TyrianMouse.is_button_press(event):
		return
	if event.button_index == MOUSE_BUTTON_LEFT:
		_page_key(TyrianControls.LEFT if event.position.x < X_CENTER else TyrianControls.RIGHT)
	elif event.button_index == MOUSE_BUTTON_RIGHT:
		_page_key(TyrianControls.BACK)


## Choice of a topic (JE_helpSystem): the last one ("Exit") returns to the title.
func _choose_topic(sel: int) -> void:
	selected_index = sel
	Sfx.play(Sfx.SELECT)
	topic = selected_index + 2
	if selected_index == TOPIC_COUNT - 2:
		_leave()
		return
	page = TOPIC_START[topic - 1]
	_show_page()


func _leave() -> void:
	idle = false
	_busy = true
	await fade_black(15)
	if embedded:
		closed.emit()
		return
	Game.goto_scene(SCENE_TITLE)  # `restart = true` in titleScreen


## Keys of a page (helpSystemPage).
func _page_key(key: StringName) -> void:
	match key:
		TyrianControls.LEFT:
			Sfx.play(Sfx.CURSOR)
			page -= 1
		TyrianControls.RIGHT, TyrianControls.SELECT, TyrianControls.ACCEPT:
			Sfx.play(Sfx.CURSOR)
			page += 1
		TyrianControls.BACK:
			Sfx.play(Sfx.SPRING)
			topic = 1
			_back_to_menu()
			return
	_show_page()


func _show_page() -> void:
	if not _draw_page():
		_back_to_menu()


## Return from helpSystemPage: selectedIndex = topic - 1, topic 1.
func _back_to_menu() -> void:
	selected_index = topic - 1
	topic = 1
	page = 0
	mouse_pointer = MouseCursor.NORMAL
	await _draw_menu()
