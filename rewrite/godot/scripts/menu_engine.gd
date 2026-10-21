## Single menu engine for the 8-bit screens (block 20.1, ADR 0121).
##
## A single model — menu stack, typed rows, navigation, popups — for
## all the rewrite's menus. The C version has two (legacy menus.c /
## game_menu.c and menu_tree.c): their presentation differences
## become drawing styles here (`Style`), their navigation differences
## become menu options (`open`), without a second engine.
##
## Row model: menu_tree.h (MenuItemKind). Each row is a
## dictionary `{label, help, kind, ...}` built by `item()`:
## - ACTION: `action` (Callable);
## - TOGGLE: `get` / `set` (bool);
## - PICKER: `get` / `set` (int), `count`, `labels`, `popup` (picker_popup);
## - BACK: closes the menu;
## - KEYBIND: `value` (Callable -> String, key name);
## - CUSTOM: `draw(it, x, y, selected)`, `activate(it)`, `adjust(it, dir)`
##   (menu_tree.c: MENU_ITEM_CUSTOM);
## - LABEL: non-selectable text.
##
## Usage: the screen creates `MenuEngine.new(self)`, opens a menu (`open`) and
## forwards its actions (`handle_action`). Drawings write into the
## screen's 8-bit framebuffer (ADR 0020), like the C code.
##
## Screens specific to the rewrite, with no 8-bit C equivalent (side modes,
## mods): the host is a plain `Node`; the engine keeps the model and the
## navigation, the screen supplies its drawing through the `render` option.
class_name MenuEngine
extends RefCounted

enum Kind { ACTION, TOGGLE, PICKER, BACK, KEYBIND, CUSTOM, LABEL }

## C presentations reproduced pixel for pixel:
## - MENU_TREE: menu_tree.c:run_menu (Setup and submenus): header,
##   shadowed rows, value on the right, scrolling, help at the bottom;
## - TITLE: title_screen.c:titleScreen: entries pre-rendered in
##   VGAScreen2, only the chosen row is redrawn highlighted;
## - TOPICS: mainint.c:JE_helpSystem: title in large font, shadowed
##   centered topics;
## - GAME_MENU: game_menu.c:JE_itemScreen (shop): header
##   JE_drawMenuHeader centered on x = 234, choices JE_drawMenuChoices
##   ("~" = JE_dString highlight); `gap` row: vertical offset;
## - ROWS: lists of the legacy screens (mainint.c:JE_loadScreen,
##   title_screen.c:replay_browser...): each CUSTOM row draws itself
##   (`draw(it, x, y, selected)`) at `row_x`, `row_y + row_step·rank`, in
##   a window of `visible` rows; `header_draw` before the rows;
##   `window`: Callable(sel, n) -> first row, if the window does not simply
##   follow the selection (achievements: window centered on the cursor).
enum Style { MENU_TREE, TITLE, TOPICS, GAME_MENU, ROWS }

## menu_tree.c: default geometry.
const DEFAULT_X := 30
const DEFAULT_Y_START := 30
const DEFAULT_Y_START_WITH_HEADER := 36
const DEFAULT_STEP := 15
const HEADER_Y := 12
const VALUE_X_OFFSET := 160
const DEFAULT_ROW_WIDTH := 260
const MAX_VISIBLE := (185 - 13 - DEFAULT_Y_START_WITH_HEADER) / DEFAULT_STEP + 1
## menu_tree.c: ROW_HEIGHT (clickable area of a row: y - 2 .. y + 13 + 2)
## and MENU_CURSOR_CHAR.
const ROW_HEIGHT := 13
const CURSOR_CHAR := "*"

## title_screen.c:titleScreen.
const TITLE_Y_ITEMS := 88
const TITLE_H_ITEM := 12

## mainint.c:JE_helpSystem.
const TOPICS_X_CENTER := 160
const TOPICS_Y_HEADER := 30
const TOPICS_Y_ITEMS := 60
const TOPICS_DY_ITEMS := 20
const TOPICS_H_ITEM := 13

## Host 8-bit screen (null for a node-based screen, see `host`).
var screen: TyrianScreen
## Host node: `screen`, or the node-based screen that draws through the `render` option.
var host: Node
## Stack of open menus: {header, items, sel, top, style, options}.
var stack: Array = []
## Open popup (menu_tree.c:picker_popup) and deferred popup.
var _popup := {}
var _pending_popup := {}


func _init(owner_screen: Node) -> void:
	host = owner_screen
	screen = owner_screen as TyrianScreen


static func item(label: String, help: String, kind: int, extra := {}) -> Dictionary:
	var it := {"label": label, "help": help, "kind": kind}
	it.merge(extra)
	return it


## Entry into run_menu: selection on the first selectable row
## (or `options.sel`), first draw unless `options.draw == false`.
## Options (defaults = menu_tree.c):
## - style (Style) ;
## - accept: actions that activate the row ([ACCEPT]; menus.c: + SELECT);
## - move_sfx: movement sound (-1: none; menus.c: Sfx.CURSOR);
## - activate_sfx: activation sound (Sfx.CLICK; -1: none);
## - shortcuts: {action: Callable} specific to the screen; the image is
##   redrawn if the Callable returns `true`;
## - on_activate: Callable(sel) replacing per-row activation;
## - on_back: Callable called when the root closes (BACK / Esc);
## - wrap: Up/Down wrap around (true, menu_tree.c:move_selection; false:
##   achievements.c, bounded cursor);
## - redraw_on_other: an unrecognized action re-presents the image (menus.c
##   loops that redraw on every key);
## - redraw_on_horizontal: Left/Right on a row without a value
##   re-presents the image (true: run_menu; false: title, which ignores it);
## - pre_present: Callable awaited before the first `present()`;
## - background: Callable that sets the background (default: VGAScreen2 copied);
## - decorate: Callable called after the rows are drawn, before
##   `present()` (screen-specific elements: money, illustration...);
## - external: the navigation logic lives elsewhere (shop:
##   TyrianShop, Rust); the screen sets `sel` and calls `draw()`;
## - adjust_sfx: Left/Right sound on PICKER / TOGGLE (Sfx.CLICK; -1: none);
## - render: Callable() that draws the menu in place of the style (node-based
##   screen without framebuffer; no `present()`);
## - geometry of the MENU_TREE style (menu_tree.h:MenuTreeOpts, 0 or absent =
##   run_menu default): x, y_start, step, row_width; no_shadow
##   (render_default in JE_dString without shadow, "*" in front of the chosen
##   row: pause menu mainint.c:JE_inGameSetup). Empty header: no
##   header (header_key NULL), rows starting at DEFAULT_Y_START.
## Mouse (handle_mouse):
## - hit: Callable(p: Vector2i) -> rank of the row under the pointer (-1:
##   none), replaces the style's geometry (node-based screens);
## - hit_x, hit_w, row_h, hit_y0 (ROWS style): clickable area of the rows
##   (defaults: row_x, 300, row_step, row_y);
## - hover: hovering selects the row (true);
## - hover_sfx: hover sound (MENU_TREE: Sfx.CLICK; otherwise move_sfx);
## - click_activates: left click activates the row (true);
## - click_selects_first: a click on another row only selects it,
##   a click on the selected row activates it (game_menu.c);
## - click_sfx: sound of a click that selects (Sfx.CLICK);
## - right_click: "back" (default: cancel, menu_tree.c) or "click"
##   (game_menu.c: any button acts like the left one);
## - hit_row_y, hit_y0, hit_unbounded (ROWS): origin of the clickable ranks
##   (game_menu.c: 38), top area, ranks outside the visible window;
## - clamp_last (GAME_MENU): any rank beyond targets the last row.
func open(header: String, items: Array, options := {}) -> void:
	var sel: int = options.get("sel", 0)
	while sel < items.size() and not selectable(items[sel]):
		sel += 1
	stack.append({"header": header, "items": items, "sel": sel, "top": 0, "options": options})
	_hover_pointer(stack.back())
	if options.get("draw", true):
		await draw()


func current() -> Dictionary:
	return stack.back()


func selection() -> int:
	return stack.back().sel


func popup_open() -> bool:
	return not _popup.is_empty()


## menu_tree.c:item_is_selectable (SEPARATOR, LABEL excluded).
static func selectable(it: Dictionary) -> bool:
	return it.kind != Kind.LABEL


## menu_tree.c:move_selection.
static func move_selection(items: Array, sel: int, dir: int) -> int:
	var n := items.size()
	for _step in n:
		sel = (sel + dir + n) % n
		if selectable(items[sel]):
			return sel
	return sel


# --- Inputs --------------------------------------------------------------------


## A menu action (ADR 0120): navigation of run_menu or of the popup.
func handle_action(action: StringName) -> void:
	if stack.is_empty() or stack.back().options.get("external", false):
		return
	if not _popup.is_empty():
		_popup_action(action)
		return
	var m: Dictionary = stack.back()
	var opts: Dictionary = m.options
	var items: Array = m.items
	var it: Dictionary = items[m.sel] if not items.is_empty() else {}
	var move_sfx: int = opts.get("move_sfx", Sfx.CURSOR)
	if action in opts.get("shortcuts", {}):
		if opts.shortcuts[action].call() == true:
			draw()
		return
	match action:
		TyrianControls.UP, TyrianControls.DOWN:
			var dir := -1 if action == TyrianControls.UP else 1
			var previous: int = m.sel
			if opts.get("wrap", true):
				m.sel = move_selection(items, m.sel, dir)
			else:
				m.sel = clampi(m.sel + dir, 0, maxi(items.size() - 1, 0))
			# menu_tree.c:488: run_menu is silent on Up/Down (lead's decision).
			if move_sfx >= 0 and m.sel != previous and opts.get("style", Style.MENU_TREE) != Style.MENU_TREE:
				Sfx.play(move_sfx)
		TyrianControls.LEFT, TyrianControls.RIGHT:
			if (
				not it.get("kind", -1) in [Kind.PICKER, Kind.TOGGLE, Kind.CUSTOM]
				and not opts.get("redraw_on_horizontal", true)
			):
				return
			adjust_horizontal(it, -1 if action == TyrianControls.LEFT else 1, opts.get("adjust_sfx", Sfx.CLICK))
		TyrianControls.BACK:
			back()
			return
		_:
			if action in opts.get("accept", [TyrianControls.ACCEPT]):
				if opts.has("on_activate"):
					opts.on_activate.call(m.sel)
					return
				if it.kind == Kind.BACK:
					back()
					return
				# A submenu opened by activate draws its own first image.
				var depth := stack.size()
				activate(it, opts.get("activate_sfx", Sfx.CLICK))
				if stack.size() != depth:
					return
			elif not opts.get("redraw_on_other", false):
				return
	draw()


# --- Mouse -------------------------------------------------------------------


## A mouse event (game coordinates, 320×200) on the current menu or
## its popup: hover, left click, right click (no wheel, like the C code).
##
## C: menu_tree.c:run_menu (hittest_row; hover only if the mouse is
## active, `!mouseInactive`; left click = choice + activation, BACK closes,
## CUSTOM receives click(relative x, y); right click = cancel),
## title_screen.c:titleScreen (centered entries, text width),
## mainint.c:JE_helpSystem (topics), JE_loadScreen (rows),
## game_menu.c:JE_itemScreen (choices column). The wheel does not exist
## in the C code (no SDL_MOUSEWHEEL in src/): it has no effect here either.
func handle_mouse(event: InputEvent) -> void:
	if not (event is InputEventMouse) or stack.is_empty():
		return
	TyrianMouse.note(event)
	var m: Dictionary = stack.back()
	var o: Dictionary = m.options
	if o.get("external", false):
		return
	if TyrianMouse.is_wheel(event):
		return  # the C code has no wheel (no SDL_MOUSEWHEEL in src/)
	if not _popup.is_empty():
		_popup_mouse(event)
		return
	var p := Vector2i(event.position.floor())
	if event is InputEventMouseMotion:
		# Hover: only on a real movement (mouseInactive false).
		if not TyrianMouse.inactive and event.relative != Vector2.ZERO and o.get("hover", true):
			hover(m, row_at(m, p))
		return
	if not TyrianMouse.is_button_press(event):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT and o.get("right_click", "back") == "back":
		back()
		return
	if event.button_index in [MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT]:
		click(m, row_at(m, p), p)


## Row of menu `m` under point `p` (rank in `items`), -1 if none.
func row_at(m: Dictionary, p: Vector2i) -> int:
	var o: Dictionary = m.options
	var items: Array = m.items
	if o.has("hit"):
		return o.hit.call(p)
	match o.get("style", Style.MENU_TREE):
		Style.MENU_TREE:
			# menu_tree.c:hittest_row: y - 2 .. y + ROW_HEIGHT + 2, x0 .. x0 + row_w,
			# on the menu's real geometry (header or not, y_start, step, width).
			var g := tree_geometry(m)
			var n_vis: int = mini(items.size() - m.top, g.max_vis)
			for r in n_vis:
				var row_y: int = g.y_start + r * g.step
				if p.y >= row_y - 2 and p.y < row_y + ROW_HEIGHT + 2 and p.x >= g.x and p.x < g.x + g.row_width:
					return m.top + r
		Style.TITLE:
			# title_screen.c: x = xCenter - wMenuItem / 2, height hMenuItem.
			for i in items.size():
				var w := TyrianText.width(items[i].label, TyrianScreen.NORMAL_FONT)
				var x := 160 - w / 2
				var y := TITLE_Y_ITEMS + TITLE_H_ITEM * i
				if p.x >= x and p.x < x + w and p.y >= y and p.y < y + TITLE_H_ITEM:
					return i
		Style.TOPICS:
			# mainint.c:JE_helpSystem: centered topics, hMenuItem = 13.
			for i in items.size():
				var w := TyrianText.width(items[i].label, TyrianScreen.NORMAL_FONT)
				var x := TOPICS_X_CENTER - w / 2
				var y := TOPICS_Y_ITEMS + TOPICS_DY_ITEMS * i
				if p.x >= x and p.x < x + w and p.y >= y and p.y < y + TOPICS_H_ITEM:
					return i
		Style.GAME_MENU:
			# game_menu.c:mouse_in_menu_column (mouseY > 20, 170 < mouseX < 308)
			# and menu_row_under_mouse: (mouseY - 38) / 16 (C division truncated),
			# "selection > 7 -> 7" for MENU_FULL_GAME (`clamp_last`),
			# "selection == menuChoices + 1 -> menuChoices" when the last
			# row is offset (MENU_PLAY_NEXT_LEVEL, `gap`).
			if p.x <= 170 or p.x >= 308 or p.y <= 20:
				return -1
			var n := items.size()
			var idx := (p.y - 38) / int(o.get("row_step", 16))
			if idx < 0 or n == 0:
				return -1
			if o.get("clamp_last", false):
				idx = mini(idx, n - 1)
			elif idx == n and int(items[n - 1].get("gap", 0)) > 0:
				idx = n - 1
			return idx if idx < n else -1
		Style.ROWS:
			var step := int(o.get("row_step", 0))
			if step <= 0:
				return -1
			var x0 := int(o.get("hit_x", o.get("row_x", 0)))
			var y0 := int(o.get("hit_row_y", o.get("row_y", 0)))
			var h := int(o.get("row_h", step))
			if p.x < x0 or p.x >= x0 + int(o.get("hit_w", 300)) or p.y < int(o.get("hit_y0", y0)):
				return -1
			var rel := p.y - y0
			# Division truncated toward zero, like the C `(mouseY - 38) / step`.
			var r := rel / step
			if r < 0 or rel >= 0 and rel - r * step >= h:
				return -1
			if r >= int(o.get("visible", items.size())) and not o.get("hit_unbounded", false):
				return -1
			var i: int = m.top + r
			return i if i < items.size() else -1
	return -1


## Hover over row `i`: it becomes the selection (sound, image).
func hover(m: Dictionary, i: int) -> void:
	if i < 0 or i >= m.items.size() or i == m.sel or not selectable(m.items[i]):
		return
	m.sel = i
	var sfx: int = m.options.get("hover_sfx", _default_hover_sfx(m))
	if sfx >= 0:
		Sfx.play(sfx)
	draw()


func _default_hover_sfx(m: Dictionary) -> int:
	# menu_tree.c:536-538, 580-584: hover and click of a MENU_TREE menu = S_CLICK.
	if m.options.get("style", Style.MENU_TREE) == Style.MENU_TREE:
		return Sfx.CLICK
	return m.options.get("move_sfx", Sfx.CURSOR)


## Click on row `i` at point `p` (menu_tree.c:run_menu, branch
## SDL_BUTTON_LEFT; title_screen.c, JE_loadScreen: action = true).
func click(m: Dictionary, i: int, p: Vector2i) -> void:
	var o: Dictionary = m.options
	if i < 0 or i >= m.items.size() or not selectable(m.items[i]):
		return
	var changed: bool = i != m.sel
	if changed and o.get("click_selects_first", false):
		# game_menu.c: a click on another row selects it (S_CLICK).
		m.sel = i
		Sfx.play(o.get("click_sfx", Sfx.CLICK))
		draw()
		return
	if changed:
		# menu_tree.c: S_CLICK; title, topics, JE_loadScreen: S_CURSOR.
		m.sel = i
		var sfx: int = o.get("hover_sfx", _default_hover_sfx(m))
		if sfx >= 0:
			Sfx.play(sfx)
	if not o.get("click_activates", true):
		draw()
		return
	if o.has("on_activate"):
		o.on_activate.call(m.sel)
		return
	var it: Dictionary = m.items[m.sel]
	if it.kind == Kind.BACK:
		back()
		return
	if it.kind == Kind.CUSTOM and it.has("click"):
		# menu_tree.c: custom.click(item, lastmouse_x - x0, lastmouse_y - row_y).
		var g := tree_geometry(m)
		var row_y: int = g.y_start + (m.sel - m.top) * g.step
		it.click.call(it, p.x - g.x, p.y - row_y)
		draw()
		return
	var depth := stack.size()
	activate(it, o.get("activate_sfx", Sfx.CLICK))
	if stack.size() != depth:
		return
	draw()


## menu_tree.c:run_menu, hover of each loop turn: when the mouse
## is active, the row under the pointer is chosen as soon as a menu is entered
## (or on return from a submenu), without movement.
func _hover_pointer(m: Dictionary) -> void:
	if TyrianMouse.inactive or TyrianMouse.position.x < 0:
		return
	if (
		m.options.get("style", Style.MENU_TREE) != Style.MENU_TREE
		or m.options.has("render")
		or m.options.get("external", false)
		or not m.options.get("hover", true)
	):
		return
	var i := row_at(m, TyrianMouse.position)
	if i >= 0 and i != m.sel and selectable(m.items[i]):
		m.sel = i
		Sfx.play(Sfx.CLICK)  # menu_tree.c:580-584


## menu_tree.c:picker_popup: hover, left click (choice confirmed), right click
## (cancel). Box of `w` px, rows of 13 px every 15 px.
func _popup_mouse(event: InputEvent) -> void:
	var p := Vector2i(event.position.floor())
	var hit := -1
	for i in int(_popup.it.count):
		var y: int = _popup.y + 2 + i * 15
		if p.x >= _popup.x and p.x < _popup.x + _popup.w and p.y >= y and p.y < y + 13:
			hit = i
			break
	if event is InputEventMouseMotion:
		if hit >= 0 and hit != _popup.sel and not TyrianMouse.inactive:
			_popup.sel = hit
			Sfx.play(Sfx.CLICK)  # menu_tree.c:772
			_popup_draw()
		return
	if not TyrianMouse.is_button_press(event):
		return
	if event.button_index == MOUSE_BUTTON_RIGHT:
		_popup_action(TyrianControls.BACK)
	elif event.button_index == MOUSE_BUTTON_LEFT and hit >= 0:
		if hit != _popup.sel:
			_popup.sel = hit
			Sfx.play(Sfx.CLICK)
		_popup_action(TyrianControls.ACCEPT)


## menu_tree.c:activate (TOGGLE flips, PICKER without popup advances one notch).
func activate(it: Dictionary, sfx := Sfx.CLICK) -> void:
	if sfx >= 0:
		Sfx.play(sfx)
	match it.kind:
		Kind.ACTION:
			it.action.call()
		Kind.TOGGLE:
			it.set.call(not it.get.call())
		Kind.PICKER:
			if it.get("popup", false):
				# Deferred: picker_popup opens after the menu is drawn.
				_pending_popup = it
			else:
				it.set.call((it.get.call() + 1) % it.count)
		Kind.CUSTOM:
			# List rows (achievements, saves...) without an action: nothing happens, like the C code.
			if it.has("activate"):
				it.activate.call(it)


## menu_tree.c:adjust_horizontal.
func adjust_horizontal(it: Dictionary, dir: int, sfx := Sfx.CLICK) -> void:
	match it.get("kind", -1):
		Kind.PICKER:
			it.set.call((it.get.call() + dir + it.count) % it.count)
			if sfx >= 0:
				Sfx.play(sfx)
		Kind.TOGGLE:
			it.set.call(not it.get.call())
			if sfx >= 0:
				Sfx.play(sfx)
		Kind.CUSTOM:
			if it.has("adjust"):
				it.adjust.call(it, dir)


## Exit from run_menu: back to the parent menu (redrawn) or the root's
## `on_back`.
func back() -> void:
	var m: Dictionary = stack.pop_back()
	if not stack.is_empty():
		_hover_pointer(stack.back())
		draw()
		return
	stack.append(m)  # the root remains the displayed state until the screen changes
	if m.options.has("on_back"):
		m.options.on_back.call()


# --- Drawing -------------------------------------------------------------------


## "Draw" part of the current menu's loop, then JE_showVGA.
func draw() -> void:
	var m: Dictionary = stack.back()
	render()
	if m.options.has("pre_present"):
		var pre: Callable = m.options.pre_present
		m.options.erase("pre_present")
		await pre.call()
	if screen != null:
		screen.present()
	host.set("idle", true)
	# run_menu: the deferred popup opens right after JE_showVGA, over
	# the menu image it has just presented.
	if not _pending_popup.is_empty():
		var p := _pending_popup
		_pending_popup = {}
		_popup_begin(p)


## Drawing of the current menu in VGAScreen, without `present()` (screens whose
## first image goes through a fade).
func render() -> void:
	var m: Dictionary = stack.back()
	if m.options.has("render"):
		m.options.render.call()
		return
	match m.options.get("style", Style.MENU_TREE):
		Style.MENU_TREE:
			_draw_menu_tree(m)
		Style.TITLE:
			_draw_title(m)
		Style.TOPICS:
			_draw_topics(m)
		Style.GAME_MENU:
			_draw_game_menu(m)
		Style.ROWS:
			_draw_rows(m)
	if m.options.has("decorate"):
		m.options.decorate.call()


## Value suffix of render_default (TOGGLE, PICKER, KEYBIND), truncated
## beyond 16 characters ("%.15s.") like the C code.
static func value_suffix(it: Dictionary) -> String:
	var s := ""
	match it.kind:
		Kind.TOGGLE:
			s = TyrianText.L("on" if it.get.call() else "off")
		Kind.PICKER:
			s = TyrianText.L(it.labels[it.get.call()])
		Kind.KEYBIND:
			s = it.value.call()
		_:
			return ""
	if s.length() > 16:
		s = s.substr(0, 15) + "."
	return s


## menu_tree.c:run_menu, geometry (x0, y_start, step, row_w): menu
## options, run_menu defaults otherwise.
static func tree_geometry(m: Dictionary) -> Dictionary:
	var o: Dictionary = m.options
	var y_default := DEFAULT_Y_START_WITH_HEADER if str(m.header) != "" else DEFAULT_Y_START
	var y_start := int(o.get("y_start", 0))
	var step := int(o.get("step", 0))
	var g := {
		"x": int(o.get("x", 0)) if int(o.get("x", 0)) != 0 else DEFAULT_X,
		"y_start": y_start if y_start != 0 else y_default,
		"step": step if step != 0 else DEFAULT_STEP,
		"row_width": int(o.get("row_width", 0)) if int(o.get("row_width", 0)) != 0 else DEFAULT_ROW_WIDTH,
	}
	# max_vis = (help_strip_y - ROW_HEIGHT - y_start) / step + 1, at least 3.
	g["max_vis"] = maxi((185 - ROW_HEIGHT - g.y_start) / g.step + 1, 3)
	return g


## menu_tree.c:render_default with opts.no_shadow (and pause_draw_label of
## mainint.c): JE_dString with fixed value, "*" in front of the chosen row.
func draw_label_no_shadow(x: int, y: int, label: String, selected: bool) -> void:
	var s := screen
	if selected:
		# Intentional divergence (no. 50): the normal-font "*" that
		# mainint.c:pause_draw_label draws is only 2×2 pixels, invisible on
		# screen (checked on the C c-final); we draw it with the small
		# font (5×5) and lighten the label (JE_dString's "~").
		s.vga.dstring(s.VGA_SCREEN, x - 9, y + 2, "~" + CURSOR_CHAR + "~", s.SMALL_FONT)
		s.vga.dstring(s.VGA_SCREEN, x, y, "~" + label + "~", s.NORMAL_FONT)
	else:
		s.vga.dstring(s.VGA_SCREEN, x, y, label, s.NORMAL_FONT)


## Value of a pause row: lightened on the chosen row like its
## label (divergence no. 50), without "*".
func draw_value_no_shadow(x: int, y: int, text: String, selected: bool) -> void:
	var s := screen
	s.vga.dstring(s.VGA_SCREEN, x, y, ("~" + text + "~") if selected else text, s.NORMAL_FONT)


## menu_tree.c:run_menu (render_default, render_item, indicators, help).
func _draw_menu_tree(m: Dictionary) -> void:
	var items: Array = m.items
	var s := screen
	var g := tree_geometry(m)
	var x0: int = g.x
	var no_shadow: bool = m.options.get("no_shadow", false)
	_background(m)  # vgascreen2_pre_render
	if str(m.header) != "":
		TyrianText.draw_font_hv_shadow(
			s, 160, HEADER_Y, TyrianText.L(m.header), s.NORMAL_FONT, s.CENTERED, 15, -4, false, 2
		)
	# Scrolling window: max_vis = (185 - ROW_HEIGHT - y_start) / step + 1
	# = 10 rows; `top` follows the selection (Key Bindings: 13, Menu Keys: 15).
	if m.sel < m.top:
		m.top = m.sel
	elif m.sel >= m.top + g.max_vis:
		m.top = m.sel - g.max_vis + 1
	var n_vis: int = mini(items.size() - m.top, g.max_vis)
	for i in range(m.top, m.top + n_vis):
		var it: Dictionary = items[i]
		var y: int = g.y_start + (i - m.top) * g.step
		if it.kind == Kind.CUSTOM:  # render_item: CUSTOM drawing
			it.draw.call(it, x0, y, i == m.sel)
			continue
		var suffix := value_suffix(it)
		if no_shadow:
			draw_label_no_shadow(x0, y, TyrianText.L(it.label), i == m.sel)
			if suffix != "":
				s.vga.dstring(s.VGA_SCREEN, x0 + VALUE_X_OFFSET, y, suffix, s.NORMAL_FONT)
			continue
		# render_default: value -3, +2 on the chosen row.
		var value: int = -3 + (2 if i == m.sel else 0)
		TyrianText.draw_font_hv_shadow(s, x0, y, TyrianText.L(it.label), s.NORMAL_FONT, s.LEFT, 15, value, false, 2)
		if suffix != "":
			TyrianText.draw_font_hv_shadow(
				s, x0 + VALUE_X_OFFSET, y, suffix, s.NORMAL_FONT, s.LEFT, 15, value, false, 2
			)
	# Scroll indicators at x0 + row_w + 4.
	if m.top > 0:
		TyrianText.text_shade(s, x0 + g.row_width + 4, g.y_start, "^", 15, 4, TyrianText.PART_SHADE)
	if m.top + n_vis < items.size():
		TyrianText.text_shade(
			s, x0 + g.row_width + 4, g.y_start + (n_vis - 1) * g.step, "v", 15, 4, TyrianText.PART_SHADE
		)
	# "else if (root[sel].help_key)": no help for a row without a key.
	if str(items[m.sel].help) != "":
		TyrianText.text_shade(s, 20, 190, TyrianText.L(items[m.sel].help), 15, 4, TyrianText.PART_SHADE)


## menu_tree.c:hittest_row: rank of the visible row under (x, y), -1 otherwise.
func hittest_row(x: int, y: int) -> int:
	var m: Dictionary = stack.back()
	var g := tree_geometry(m)
	var n_vis: int = mini(m.items.size() - m.top, g.max_vis)
	for i in n_vis:
		var row_y: int = g.y_start + i * g.step
		if y >= row_y - 2 and y < row_y + ROW_HEIGHT + 2 and x >= g.x and x < g.x + g.row_width:
			return m.top + i
	return -1


## title_screen.c:titleScreen, top of the loop: highlight of the chosen entry
## on the pre-rendered background of VGAScreen2 (`render_title_items`).
func _draw_title(m: Dictionary) -> void:
	var s := screen
	_background(m)
	TyrianText.draw_font_hv(
		s, 160, TITLE_Y_ITEMS + TITLE_H_ITEM * m.sel, m.items[m.sel].label, s.NORMAL_FONT, s.CENTERED, 15, -1
	)


## title_screen.c:titleScreen, `restart` branch: entries with dark outline
## (four offsets, value -10) then text (-3), drawn once.
func render_title_items(items: Array) -> void:
	var s := screen
	for i in items.size():
		var t: String = items[i].label
		var x := 160 - TyrianText.width(t, s.NORMAL_FONT) / 2
		var y := TITLE_Y_ITEMS + TITLE_H_ITEM * i
		TyrianText.draw_font_hv(s, x - 1, y - 1, t, s.NORMAL_FONT, s.LEFT, 15, -10)
		TyrianText.draw_font_hv(s, x + 1, y + 1, t, s.NORMAL_FONT, s.LEFT, 15, -10)
		TyrianText.draw_font_hv(s, x + 1, y - 1, t, s.NORMAL_FONT, s.LEFT, 15, -10)
		TyrianText.draw_font_hv(s, x - 1, y + 1, t, s.NORMAL_FONT, s.LEFT, 15, -10)
		TyrianText.draw_font_hv(s, x, y, t, s.NORMAL_FONT, s.LEFT, 15, -3)


## mainint.c:JE_helpSystem (topic 1: topic menu).
func _draw_topics(m: Dictionary) -> void:
	var s := screen
	_background(m)
	TyrianText.draw_font_hv_shadow(
		s, TOPICS_X_CENTER, TOPICS_Y_HEADER, m.header, s.LARGE_FONT, s.CENTERED, 15, -3, false, 2
	)
	for i in m.items.size():
		var value := -3 + (2 if i == m.sel else 0)
		TyrianText.draw_font_hv_shadow(
			s,
			TOPICS_X_CENTER,
			TOPICS_Y_ITEMS + TOPICS_DY_ITEMS * i,
			m.items[i].label,
			s.NORMAL_FONT,
			s.CENTERED,
			15,
			value,
			false,
			2
		)


## game_menu.c:JE_itemScreen, menus MENU_FULL_GAME / MENU_UPGRADES:
## JE_drawMenuHeader then JE_drawMenuChoices (x = 166, y = 38 + 16·i).
func _draw_game_menu(m: Dictionary) -> void:
	var s := screen
	_background(m)
	draw_game_menu_header(m.header)
	for i in m.items.size():
		var it: Dictionary = m.items[i]
		var y: int = 38 + 16 * i + int(it.get("gap", 0))
		s.vga.dstring(s.VGA_SCREEN, 166, y, ("~" if i == m.sel else "") + it.label, s.NORMAL_FONT)


## game_menu.c:JE_drawMenuHeader: centered on x = 234, small font if too
## wide (the C's FONT_SHAPES = table 0 = LARGE_FONT here, SMALL_FONT_SHAPES =
## NORMAL_FONT).
func draw_game_menu_header(text: String) -> void:
	var s := screen
	var font := s.LARGE_FONT
	var w := TyrianText.width(text, font)
	if w > 318 - 152:
		font = s.NORMAL_FONT
		w = TyrianText.width(text, font)
	var x := 234 - w / 2
	if x + w > 318:
		x = 318 - w
	x = maxi(x, 152)
	s.vga.dstring(s.VGA_SCREEN, x, 10, text, font)


## Lists with rows drawn by the screen; `top` follows the selection.
func _draw_rows(m: Dictionary) -> void:
	var o: Dictionary = m.options
	_background(m)
	if o.has("header_draw"):
		o.header_draw.call()
	var visible: int = o.get("visible", m.items.size())
	if o.has("window"):
		m.top = o.window.call(m.sel, m.items.size())
	elif m.sel < m.top:
		m.top = m.sel
	elif m.sel >= m.top + visible:
		m.top = m.sel - visible + 1
	for i in range(m.top, mini(m.items.size(), m.top + visible)):
		var it: Dictionary = m.items[i]
		it.draw.call(
			it, int(o.get("row_x", 0)), int(o.get("row_y", 0)) + (i - m.top) * int(o.get("row_step", 0)), i == m.sel
		)


func _background(m: Dictionary) -> void:
	if m.options.has("background"):
		m.options.background.call()
	else:
		screen.vga.copy(screen.VGA_SCREEN, screen.VGA_SCREEN2)
		# Scenery text leftovers (see TyrianScreen.BACKDROP_MASK_LEFT). Not on
		# the title, where the planet occupies the area.
		if m.options.get("style", Style.MENU_TREE) != Style.TITLE:
			var mask_left: int = TyrianScreen.BACKDROP_MASK_LEFT
			var mask_top: int = TyrianScreen.BACKDROP_MASK_TOP
			screen.vga.fill_rectangle_xy(screen.VGA_SCREEN, mask_left, mask_top, 319, 199, 0)


# --- picker_popup (menu_tree.c) ----------------------------------------------


## menu_tree.c:picker_popup (called from activate: row_x = row_y = -1).
func _popup_begin(it: Dictionary) -> void:
	var count: int = it.count
	var w_box := 132
	var x_box := 320 - w_box - 12
	var h_box := 15 * count + 2
	if h_box > 200 - 20:
		h_box = 200 - 20
	var y_box := 40
	if y_box + h_box > 200 - 10:
		y_box = 200 - 10 - h_box
	if y_box < 10:
		y_box = 10
	var sel: int = it.get.call()
	if sel < 0 or sel >= count:
		sel = 0
	_popup = {
		"it": it,
		"sel": sel,
		"original": sel,
		"x": x_box,
		"y": y_box,
		"w": w_box,
		"h": h_box,
		"snapshot": screen.vga.indices(screen.VGA_SCREEN)
	}
	_popup_draw()


## One iteration of the picker_popup loop: action then drawing.
func _popup_action(action: StringName) -> void:
	var count: int = _popup.it.count
	match action:
		# menu_tree.c:750-753: Up/Down move the selection without sound.
		TyrianControls.UP:
			_popup.sel = (_popup.sel - 1 + count) % count
		TyrianControls.DOWN:
			_popup.sel = (_popup.sel + 1) % count
		TyrianControls.ACCEPT:
			var it: Dictionary = _popup.it
			var changed: bool = _popup.sel != _popup.original
			var sel: int = _popup.sel
			_popup = {}
			# menu_tree.c:821-825: S_CLICK only if the value changes.
			if changed:
				it.set.call(sel)
				Sfx.play(Sfx.CLICK)
			draw()
			return
		TyrianControls.BACK:
			_popup = {}
			draw()
			return
		_:
			pass  # loop without input: the image is re-presented identical
	_popup_draw()


func _popup_draw() -> void:
	var s := screen
	s.vga.set_indices(s.VGA_SCREEN, _popup.snapshot)
	var x: int = _popup.x
	var y: int = _popup.y
	var w: int = _popup.w
	var h: int = _popup.h
	# Triple frame then background 224 (JE_rectangle ×3, fill_rectangle_wh).
	s.vga.rectangle(s.VGA_SCREEN, x - 5, y - 3, x + w + 5 - 1, y + h + 3 - 1, 248)
	s.vga.rectangle(s.VGA_SCREEN, x - 4, y - 4, x + w + 4 - 1, y + h + 4 - 1, 250)
	s.vga.rectangle(s.VGA_SCREEN, x - 3, y - 5, x + w + 3 - 1, y + h + 5 - 1, 248)
	s.vga.fill_rectangle_xy(s.VGA_SCREEN, x - 2, y - 2, x - 2 + w + 4 - 1, y - 2 + h + 4 - 1, 224)
	var labels: Array = _popup.it.labels
	for i in _popup.it.count:
		TyrianText.draw_font_hv_shadow(
			s,
			x + 2,
			y + 2 + i * 15,
			TyrianText.L(labels[i]),
			s.NORMAL_FONT,
			s.LEFT,
			15,
			-3 + (2 if i == _popup.sel else 0),
			false,
			2
		)
	s.present()
