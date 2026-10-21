#!/usr/bin/env python3
"""Player journeys with verified effects (rewrite/scripts/player-journeys.sh).

Each journey plays the Linux package like a player (real keys and real
X clicks, see journey_driver.py) and checks, after each action, its EFFECT
read back from the game: money debited by the price, item mounted, option changed and
kept, save file written then read back. Each step is recorded:
action, expected effect, observed effect, PASS/FAIL, screenshot. A journey that
can no longer progress stops on a FAIL; the following ones start from a fresh game.

  player_journeys.py --package DIR --out DIR [--core] [--only a,b] [--sizes 1280x684,1920x1080]
                     [--profile DIR] [--rev SHA]
"""

import argparse
import json
import shutil
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from journey_driver import USER_DIR, Game, JourneyFailure, stop_games_on_signal  # noqa: E402

# Mapping shop category -> equipment slot (game.gd,
# NEW_GAME_LOADOUT: ship, front, power, rear, power, shield,
# generator, left sidekick, right sidekick, special, money).
CATEGORY_SLOT = {2: 0, 3: 1, 4: 3, 5: 5, 6: 6, 7: 7, 8: 8}
CATEGORY_NAME = {
    2: "ship",
    3: "front gun",
    4: "rear gun",
    5: "shield",
    6: "generator",
    7: "left sidekick",
    8: "right sidekick",
}
# Purchase order requested by the map.
BUY_ORDER = [3, 4, 7, 8, 6, 5, 2]
SHOP_MAIN = 0
SHOP_UPGRADES = 1
SHOP_NEXT_LEVEL = 3
SHOP_CATEGORY = 4
SHOP_CUBES = 7
SHOP_CUBE_READ = 8
INITIAL_CASH_EP1 = 10000


class Report:
    """Recorded steps (one line per verified effect) and Markdown report.

    One `JourneyLog` per journey: two journeys run at the same time (two
    games at most), each records its steps; the report groups them by journey."""

    def __init__(self, out):
        self.out = Path(out)
        self.shots = self.out / "shots"
        self.shots.mkdir(parents=True, exist_ok=True)
        self.logs = []
        self.lock = threading.Lock()
        self.jsonl = open(self.out / "steps.jsonl", "w")

    def journey(self, name, game):
        log = JourneyLog(self, name, game)
        with self.lock:
            self.logs.append(log)
        return log

    @property
    def rows(self):
        return [r for log in self.logs for r in log.rows]

    def write(self, path, meta):
        rows = self.rows
        for i, r in enumerate(rows, 1):
            r["n"] = i
        by_journey = {}
        for r in rows:
            j = by_journey.setdefault(r["journey"], [0, 0])
            j[0 if r["status"] == "PASS" else 1] += 1
        fails = [r for r in rows if r["status"] == "FAIL"]
        lines = [
            f"# Player journeys — {meta['rev']}",
            "",
            f"Package: `{meta['package']}`  ",
            f"Duration: {meta['duration']}  ",
            f"Steps: {len(rows)}, PASS {len(rows) - len(fails)}, FAIL {len(fails)}",
            "",
            "| Journey | PASS | FAIL |",
            "|---|---:|---:|",
        ]
        for j, (p, f) in by_journey.items():
            lines.append(f"| {j} | {p} | {f} |")
        lines += ["", "## Failures", ""]
        if not fails:
            lines.append("None.")
        for r in fails:
            lines.append(
                f"- **[{r['journey']}] {r['action']}** — expected: {r['expected']}; observed: "
                f"{r['observed']}" + (f"; screenshot `shots/{r['shot']}`" if r["shot"] else "")
            )
        lines += [
            "",
            "## All steps",
            "",
            "| # | Journey | Action | Expected | Observed | Status | Screenshot |",
            "|---:|---|---|---|---|---|---|",
        ]
        for r in rows:
            cells = [
                str(r["n"]),
                r["journey"],
                r["action"],
                r["expected"],
                r["observed"],
                r["status"],
                f"shots/{r['shot']}" if r["shot"] else "",
            ]
            lines.append("| " + " | ".join(c.replace("|", "/").replace("\n", " ") for c in cells) + " |")
        Path(path).write_text("\n".join(lines) + "\n")


class JourneyLog:
    """Steps of a journey: action, expected effect, observed effect, PASS/FAIL, screenshot."""

    def __init__(self, report, name, game):
        self.report = report
        self.journey = name
        self.game = game
        self.shots = report.shots
        self.rows = []

    def check(self, action, expected, observed, ok, shot=None):
        ok = bool(ok)
        name = ""
        if self.game is not None and (shot or not ok) and self.game.alive():
            name = f"{self.journey}-{len(self.rows) + 1:03d}.png"
            self.game.screenshot(self.shots / name)
        row = {
            "journey": self.journey,
            "action": action,
            "expected": expected,
            "observed": str(observed),
            "status": "PASS" if ok else "FAIL",
            "shot": name,
        }
        self.rows.append(row)
        with self.report.lock:
            self.report.jsonl.write(json.dumps(row, ensure_ascii=False) + "\n")
            self.report.jsonl.flush()
            print(f"  {row['status']}  [{self.journey}] {action} -> {observed}", flush=True)
        return ok

    def fail(self, action, expected, observed):
        return self.check(action, expected, observed, False)


# --- Common navigation --------------------------------------------------------


def menu_of(s, pause=False):
    if pause:
        return s.get("level", {}).get("pause_menu_state", {})
    return s.get("menu", {})


def choose_menu_row(g, rep, mode, index, what, pause=False, activate=True):
    """Chooses row `index` of the open menu (common engine) then activates it.

    Keyboard: Down/Up to the row (the selection must follow each
    key), Enter. Mouse: hover (the selection must follow the mouse),
    left click."""
    s = g.state()
    m = menu_of(s, pause)
    if not m:
        raise JourneyFailure(f'no open menu to choose "{what}"')
    # The screen must be waiting for an input (entry fade finished): otherwise the key is lost.
    if not pause and "idle" in s.get("screen", {}):
        g.wait(lambda st: st.get("screen", {}).get("idle"), 6, f'screen ready before "{what}"')
        time.sleep(0.2)
    if mode == "keyboard":
        n = len(m["labels"])
        for _ in range(3 * (n + 1)):
            sel = menu_of(g.state(), pause).get("sel")
            if sel == index:
                break
            g.key("Down" if (index - sel) % n <= n // 2 else "Up", pause=0.1)
            try:
                g.wait(
                    lambda st, before=sel: menu_of(st, pause).get("sel") != before,
                    1.5,
                    "selection moved",
                )
            except JourneyFailure:
                pass
        sel = menu_of(g.state(), pause).get("sel")
        if not rep.check(f'Down/Up to "{what}"', f"row {index} chosen", f"row {sel}", sel == index):
            raise JourneyFailure(f'the selection does not reach "{what}"')
        if activate:
            g.key("Return")
    else:
        hit = m.get("hits", {}).get(str(index))
        if hit is None:
            raise JourneyFailure(f'"{what}" has no clickable area')
        g.move(hit[0] + 1, hit[1])
        g.move(*hit)
        try:
            st = g.wait(lambda st: menu_of(st, pause).get("sel") == index, 3, f'hover over "{what}"')
        except JourneyFailure:
            rep.fail(
                f'hover over "{what}"',
                f"row {index} chosen",
                f"row {menu_of(g.state(), pause).get('sel')}",
            )
            raise
        rep.check(
            f'hover over "{what}"',
            f"row {index} chosen",
            f"row {menu_of(st, pause).get('sel')}",
            True,
        )
        if activate:
            g.x("click", "1")
            time.sleep(0.4)


def choose_menu_row_safe(g, rep, mode, index, what, pause=False, activate=True):
    choose_menu_row(g, rep, mode, index, what, pause, activate)


def shop_state(g):
    return g.state().get("shop", {})


def shop_goto_row(g, rep, mode, row, what, activate=True):
    """Shop (TyrianShop): row `row` (2 = first) then Enter / click."""
    sp = shop_state(g)
    if mode == "keyboard":
        # One key at a time, wait for the selection to move (a key may fall
        # during the weapon preview animation); never more than 3 tries per step.
        for _ in range(3 * (int(sp.get("menu_choices", 7)) + 2)):
            sp = shop_state(g)
            if sp.get("sel") == row:
                break
            before = sp.get("sel")
            g.key("Down" if row > before else "Up", pause=0.1)
            try:
                g.wait(lambda st, before=before: st.get("shop", {}).get("sel") != before, 1.5, "selection moved")
            except JourneyFailure:
                pass
        sp = shop_state(g)
        if sp.get("sel") != row:
            rep.fail(f'Down/Up to "{what}"', f"row {row}", f"row {sp.get('sel')}")
            raise JourneyFailure(f'shop: "{what}" unreachable')
        if activate:
            g.key("Return")
    else:
        hit = sp.get("hits", {}).get(str(row))
        if hit is None:
            rep.fail(f'mouse on "{what}"', "clickable area", "none")
            raise JourneyFailure(f'shop: "{what}" has no clickable area')
        g.move(hit[0] + 1, hit[1])
        g.move(*hit)
        try:
            g.wait(lambda st: st.get("shop", {}).get("sel") == row, 3, f'hover over "{what}"')
        except JourneyFailure:
            rep.fail(f'hover over "{what}"', f"row {row} chosen", f"row {shop_state(g).get('sel')}")
            raise
        if activate:
            # Shop (game_menu.c): a click on the already chosen row activates it.
            g.x("click", "1")
            time.sleep(0.4)


def wait_title_menu(g):
    """Title ready to receive a key (intro logos passed)."""
    return g.wait(
        lambda s: (
            s.get("scene_class") == "title.gd" and s.get("menu", {}).get("labels") and s.get("screen", {}).get("idle")
        ),
        25,
        "title menu",
    )


def new_game(g, rep, mode, kind_index=0, difficulty_index=1, expect_shop=True):
    """Title -> Start New Game -> mode -> episode 1 -> difficulty."""
    wait_title_menu(g)
    choose_menu_row_safe(g, rep, mode, 0, "Start New Game")
    g.wait(
        lambda s: s.get("scene_class") == "new_game.gd" and s.get("menu", {}).get("labels"),
        10,
        "mode choice",
    )
    labels = g.state()["menu"]["labels"]
    choose_menu_row_safe(g, rep, mode, kind_index, labels[kind_index])
    if kind_index in (0, 1, 4):
        g.wait(
            lambda s: (
                s.get("menu", {}).get("labels", [""])[0].startswith("Episode") or s.get("scene_class") != "new_game.gd"
            ),
            10,
            "episode choice",
        )
        if g.state().get("scene_class") == "new_game.gd":
            choose_menu_row_safe(g, rep, mode, 0, "Episode 1")
            g.wait(
                lambda s: s.get("menu", {}).get("labels", [""])[0] == "Easy" or s.get("scene_class") != "new_game.gd",
                10,
                "difficulty choice",
            )
            if g.state().get("scene_class") == "new_game.gd":
                choose_menu_row_safe(g, rep, mode, difficulty_index, ["Easy", "Normal", "Hard"][difficulty_index])
    if expect_shop:
        s = g.wait(
            lambda st: st.get("scene_class") == "shop.gd" and st.get("screen", {}).get("idle"),
            20,
            "shop",
        )
        return s
    return g.state()


# --- Journeys ------------------------------------------------------------------


def journey_shop(g, rep, mode, difficulty_index):
    s = new_game(g, rep, mode, 0, difficulty_index)
    diff = s["game"]["difficulty"]
    rep.check(
        f"new game ({['Easy', 'Normal', 'Hard'][difficulty_index]})",
        f"shop open, cash {INITIAL_CASH_EP1}, difficulty {difficulty_index + 1}",
        f"cash {s['game']['cash']}, difficulty {diff}",
        s["game"]["cash"] == INITIAL_CASH_EP1 and diff == difficulty_index + 1,
        shot=True,
    )
    mute_check(g, rep, "shop")
    # Core (before each push): one front gun and one sidekick; full batch: each category.
    for cat in [3, 7] if g.light else BUY_ORDER:
        buy_in_category(g, rep, mode, cat)
    if g.light:
        return
    sell_rear(g, rep, mode)
    ship_specs(g, rep, mode)
    data_cube(g, rep, mode)


def open_category(g, rep, mode, cat):
    sp = shop_state(g)
    if sp.get("menu") == SHOP_MAIN:
        shop_goto_row(g, rep, mode, 4, "Upgrade Ship")
        g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES, 4, "menu Upgrade Ship")
    shop_goto_row(g, rep, mode, cat, f"category {CATEGORY_NAME[cat]}")
    return g.wait(
        lambda st: st.get("shop", {}).get("menu") == SHOP_CATEGORY and st["shop"].get("category") == cat,
        4,
        f"list {CATEGORY_NAME[cat]}",
    )["shop"]


def buy_in_category(g, rep, mode, cat):
    name = CATEGORY_NAME[cat]
    try:
        sp = open_category(g, rep, mode, cat)
    except JourneyFailure as e:
        rep.fail(f"open {name}", "item list", str(e))
        raise
    slot = CATEGORY_SLOT[cat]
    cash0 = shop_cash_outside(g, sp)
    owned = sp["owned_item"]
    owned_value = int(sp.get("owned_value", 0))
    target = None
    for i, (item, cost) in enumerate(zip(sp["items"], sp["costs"], strict=False)):
        if item != owned and item != 0 and cost <= cash0 + owned_value:
            target = (i, item, cost)
            break
    if target is None:
        rep.check(
            f"{name}: different affordable item",
            "at least one item to buy",
            f"items {sp['items']} prices {sp['costs']}, owned {owned}, cash {cash0}: none",
            True,
        )
        g.key("Escape")
        g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES, 4, "back to Upgrade Ship")
        return
    i, item, cost = target
    row = i + 2
    shop_goto_row(g, rep, mode, row, f"{name} no. {item}")
    sp = g.settle(0.8).get("shop", {})
    how = "Enter" if mode == "keyboard" else "click"
    rep.check(
        f"{name}: {how} on item {item} ({cost})",
        "item chosen (chosen_item)",
        f"chosen {sp.get('chosen_item')}",
        sp.get("chosen_item") == item,
    )
    visible_row = row - 2 - int(sp.get("scroll") or 0)
    rep.check(
        f"{name}: {how} on item {item}",
        f"row {visible_row} shows the equipped mark on screen",
        f"marked rows {sp.get('marked_rows')}",
        visible_row in (sp.get("marked_rows") or []),
        shot=True,
    )
    done = int(sp["menu_choices"])
    shop_goto_row(g, rep, mode, done, "Done")
    s = g.wait(
        lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES,
        4,
        "back to Upgrade Ship after Done",
    )
    lo = s["shop"]["committed_loadout"]
    expected_cash = cash0 - cost + owned_value
    rep.check(
        f"{name}: Done",
        f"equipment[{slot}] = {item}",
        f"equipment[{slot}] = {lo[slot]}",
        lo[slot] == item,
    )
    rep.check(
        f"{name}: Done",
        f"cash {cash0} − {cost} + trade-in {owned_value} = {expected_cash}",
        f"cash {lo[10]}",
        lo[10] == expected_cash,
    )
    # The reopened category shows the mark on the purchased item.
    sp = open_category(g, rep, mode, cat)
    vis = sp["items"].index(item) - int(sp.get("scroll") or 0) if item in sp["items"] else -1
    rep.check(
        f"{name}: list reopened",
        f"equipped mark on row {vis} (item {item})",
        f"owned {sp['owned_item']}, marked rows {sp.get('marked_rows')}",
        sp["owned_item"] == item and vis in (sp.get("marked_rows") or []),
        shot=True,
    )
    g.key("Escape")
    s = g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES, 4, "Escape: back to Upgrade Ship")
    lo2 = s["shop"]["committed_loadout"]
    rep.check(
        f"{name}: Escape without choosing",
        "equipment and cash unchanged",
        f"equipment[{slot}] = {lo2[slot]}, cash {lo2[10]}",
        lo2[slot] == item and lo2[10] == expected_cash,
    )


def shop_goto_row_visible(g, rep, mode, row, what):
    """Shop row: with the mouse if it is visible, otherwise with the keyboard (scrolled list)."""
    if mode == "mouse" and str(row) not in shop_state(g).get("hits", {}):
        mode = "keyboard"
    shop_goto_row(g, rep, mode, row, what)


# The packs of rewrite/mods (shop-mods journey: all enabled at once).
SHIPPED_MODS = ("alien_ship_pack", "alien_weapon_pack", "bio_ships_pack", "bio_weapons_pack", "elements_weapon_pack")


def journey_shop_mods(g, rep, mode):
    """The five shipped packs enabled (--mods): all their always_available items are listed, and the
    first one of each category is bought and equipped."""
    new_game(g, rep, mode, 0, 1)
    # First mod item number per category (ships 19+, weapons 61+).
    first_mod_id = {2: 19, 3: 61, 4: 61}
    # Items of the five packs per category (ADR 0134: the list takes them all).
    mod_count = {2: 14, 3: 9, 4: 9}
    for cat in (2, 3, 4):
        name = CATEGORY_NAME[cat]
        sp = open_category(g, rep, mode, cat)
        mods = [i for i in sp["items"] if i >= first_mod_id[cat]]
        rep.check(
            f"{name}: mod items in the list",
            "at least one mod item offered",
            f"items {sp['items']}, prices {sp['costs']}",
            bool(mods),
            shot=True,
        )
        if not mods:
            raise JourneyFailure(f"no mod item for sale ({name})")
        rep.check(
            f"{name}: every pack item listed",
            f"{mod_count[cat]} mod items",
            f"{len(mods)} mod items",
            len(mods) == mod_count[cat],
        )
        cash0 = shop_cash_outside(g, sp)
        owned_value = int(sp.get("owned_value", 0))
        i = sp["items"].index(mods[0])
        cost = sp["costs"][i]
        shop_goto_row_visible(g, rep, mode, i + 2, f"mod {name} #{mods[0]}")
        g.settle(0.8)
        rep.check(
            f"mod {name}: selection",
            "mod item chosen",
            f"chosen {shop_state(g).get('chosen_item')}",
            shop_state(g).get("chosen_item") == mods[0],
            shot=True,
        )
        # "Done" is outside the visible window of a long list: keys.
        shop_goto_row(g, rep, "keyboard", int(shop_state(g)["menu_choices"]), "Done")
        s = g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES, 4, "back to Upgrade Ship after Done")
        lo = s["shop"]["committed_loadout"]
        slot = CATEGORY_SLOT[cat]
        rep.check(
            f"mod {name}: Done",
            f"equipment[{slot}] = {mods[0]}, money {cash0} - {cost} + trade-in {owned_value} = {cash0 - cost + owned_value}",
            f"equipment[{slot}] = {lo[slot]}, money {lo[10]}",
            lo[slot] == mods[0] and lo[10] == cash0 - cost + owned_value,
            shot=True,
        )
    # In the level: 60 s with the three mod items, then save, back to the title, load.
    start_level(g, rep, mode)
    play(g, 60)
    s = g.state()
    rep.check(
        "60 s of play with the mod items",
        "level still running",
        f"scene {s.get('scene_class')}",
        s.get("scene_class") == "level.gd",
        shot=True,
    )
    g.key("ctrl+shift+n")
    g.wait(lambda st: st.get("scene_class") != "level.gd", 30, "level end")
    for _ in range(30):
        s = g.state()
        if s.get("scene_class") == "shop.gd" and s.get("screen", {}).get("idle"):
            break
        g.key("Return", pause=0.8)
    s = g.state()
    if s.get("scene_class") != "shop.gd":
        raise JourneyFailure(f"shop not reached after the level (scene {s.get('scene_class')})")
    rep.check(
        "shop after the level",
        "mod equipment kept",
        f"{s['shop']['committed_loadout'][:10]}",
        s["shop"]["committed_loadout"][0] == 19
        and s["shop"]["committed_loadout"][1] == 61
        and s["shop"]["committed_loadout"][3] == 64,
        shot=True,
    )
    save_and_reload(g, rep, mode)
    saved = json.loads((g.user_dir() / "saves" / "slot_02.json").read_text())
    rep.check(
        "save file",
        "mod items written by their name",
        f"ship {saved.get('ship')}, front {saved['front_weapon']['name']}, rear {saved['rear_weapon']['name']}",
        saved.get("ship") == "Zica Ember" and not saved["front_weapon"]["name"].startswith("#"),
    )
    # Mods disabled (game restarted without --mods, same data): loading refuses cleanly.
    g.close()
    g2 = Game(g.package, g.out, (g.width, g.height), g.home, extra_args=["--no-intro"], tag=g.tag + "-without-mods")
    g2.light = True
    try:
        g2.start()
        wait_title_menu(g2)
        choose_menu_row_safe(g2, rep, mode, 1, "Load Game")
        g2.wait(
            lambda st: st.get("scene_class") == "load_screen.gd" and st.get("menu", {}).get("hits"),
            8,
            "Load Game screen",
        )
        time.sleep(1.0)
        rep.check(
            "Load Game without the mods",
            "load screen shown, game alive",
            f"scene {g2.state().get('scene_class')}",
            g2.alive() and g2.state().get("scene_class") == "load_screen.gd",
        )
        g2.screenshot(rep.shots / f"{g.tag}-without-mods.png")
    finally:
        g2.close()
        log = g2.log_path.read_text(errors="replace") if g2.log_path.exists() else ""
        errs = [ln for ln in log.splitlines() if "SCRIPT ERROR" in ln]
        rep.check("Godot log (without mods)", "no SCRIPT ERROR", f"{len(errs)}: {errs[:2]}", not errs)


def sell_rear(g, rep, mode):
    sp = open_category(g, rep, mode, 4)
    owned = sp["owned_item"]
    value = int(sp.get("owned_value", 0))
    cash0 = shop_cash_outside(g, sp)
    if 0 not in sp["items"] or owned == 0:
        rep.check(
            "rear gun sale",
            '"None" row and rear gun owned',
            f"items {sp['items']}, owned {owned}",
            False,
        )
        g.key("Escape")
        return
    row = sp["items"].index(0) + 2
    shop_goto_row(g, rep, mode, row, "rear gun: None (sale)")
    g.settle(0.6)
    shop_goto_row(g, rep, mode, int(shop_state(g)["menu_choices"]), "Done")
    s = g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_UPGRADES, 4, "back after the sale")
    lo = s["shop"]["committed_loadout"]
    rep.check(
        "rear gun sale (None then Done)",
        f"rear gun 0, cash {cash0} + trade-in {value} = {cash0 + value}",
        f"rear gun {lo[3]}, cash {lo[10]}",
        lo[3] == 0 and lo[10] == cash0 + value,
    )


def shop_cash_outside(g, sp):
    """Real money outside the list: the list inflates `cash` by the price of the owned item."""
    return int(sp["cash"]) - int(sp.get("owned_value", 0))


def back_to_shop_main(g):
    for _ in range(4):
        if shop_state(g).get("menu") == SHOP_MAIN:
            return
        g.key("Escape")
        g.settle(0.5)
    if shop_state(g).get("menu") != SHOP_MAIN:
        raise JourneyFailure("Escape does not return to the shop Game Menu")


def ship_specs(g, rep, mode):
    back_to_shop_main(g)
    shop_goto_row(g, rep, mode, 3, "Ship Specs")
    try:
        g.wait(
            lambda st: st.get("screen", {}).get("_specs_open") and st["screen"].get("idle"),
            6,
            "Ship Specs screen",
        )
        rep.check("Ship Specs", "specs screen open", "open", True, shot=True)
    except JourneyFailure as e:
        rep.fail("Ship Specs", "specs screen open", str(e))
        return
    if mode == "keyboard":
        g.key("Return")
    else:
        g.x("click", "1")
    try:
        g.wait(
            lambda st: not st.get("screen", {}).get("_specs_open") and st.get("shop", {}).get("menu") == SHOP_MAIN,
            4,
            "Ship Specs closing",
        )
        rep.check("Ship Specs: key / click", "back to Game Menu", "Game Menu", True)
    except JourneyFailure as e:
        rep.fail("Ship Specs: key / click", "back to Game Menu", str(e))


def data_cube(g, rep, mode):
    back_to_shop_main(g)
    shop_goto_row(g, rep, mode, 2, "Data")
    s = g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_CUBES, 4, "cube list")
    titles = s["shop"].get("cube_max", 0)
    if not titles:
        rep.check("Data", "at least one cube to read", "no cube", False)
        back_to_shop_main(g)
        return
    shop_goto_row(g, rep, mode, 2, "first cube")
    try:
        g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_CUBE_READ, 4, "cube reading")
        rep.check("Data: first cube", "cube text displayed", "reading open", True, shot=True)
    except JourneyFailure as e:
        rep.fail("Data: first cube", "cube text displayed", str(e))
        back_to_shop_main(g)
        return
    if mode == "mouse":
        # Wheel (maintainer, 2026-10-10): 3 notches down = 3 rows of 12 px, Read % goes up.
        s0 = shop_state(g)
        g.move(230, 100)
        for _ in range(3):
            g.x("click", "5")
            time.sleep(0.05)
        s1 = g.settle(1.0).get("shop", {})
        rep.check(
            "cube: wheel 3 notches down",
            "text moved down 36 px, Read % increased",
            f"y {s0.get('cube_y')} -> {s1.get('cube_y')}, Read {s0.get('cube_read')} % -> {s1.get('cube_read')} %",
            s1.get("cube_y") == (s0.get("cube_y") or 0) + 36 and s1.get("cube_read", 0) > s0.get("cube_read", 0),
            shot=True,
        )
        g.x("click", "4")
        s2 = g.settle(1.0).get("shop", {})
        rep.check(
            "cube: wheel 1 notch up",
            "text moved up 12 px",
            f"y {s1.get('cube_y')} -> {s2.get('cube_y')}",
            s2.get("cube_y") == (s1.get("cube_y") or 0) - 12,
        )
    if mode == "keyboard":
        g.key("Escape")
    else:
        g.click(274, 165)
    try:
        g.wait(
            lambda st: st.get("shop", {}).get("menu") == SHOP_CUBES,
            4,
            "back to the cube list",
        )
        rep.check(
            "Data: close the cube" + (" (EXIT)" if mode == "mouse" else " (Escape)"),
            "back to the list",
            "list",
            True,
        )
        if mode == "mouse":
            # Elsewhere the wheel does nothing: neither menu change nor click.
            s0 = shop_state(g)
            for b in ("5", "5", "4"):
                g.x("click", b)
                time.sleep(0.05)
            s1 = g.settle(0.6).get("shop", {})
            rep.check(
                "cube list: wheel",
                "nothing changes (menu, row)",
                f"menu {s0.get('menu')} -> {s1.get('menu')}, row {s0.get('sel')} -> {s1.get('sel')}",
                (s0.get("menu"), s0.get("sel")) == (s1.get("menu"), s1.get("sel")),
            )
    except JourneyFailure as e:
        rep.fail("Data: close the cube", "back to the list", str(e))
    back_to_shop_main(g)


def start_level(g, rep, mode):
    back_to_shop_main(g)
    shop_goto_row(g, rep, mode, 6, "Start Level")
    g.wait(lambda st: st.get("shop", {}).get("menu") == SHOP_NEXT_LEVEL, 5, "destination choice")
    shop_state(g)
    shop_goto_row(g, rep, mode, 2, "first destination")
    s = g.wait(
        lambda st: st.get("scene_class") == "level.gd" and "armor" in st.get("level", {}),
        30,
        "level",
    )
    rep.check("Start Level", "level started", f"scene {s['scene_class']}", True)
    return s


def play(g, seconds, armor_keeper=True):
    """Plays `seconds` seconds: movements and held fire; armor restored (--devmode)."""
    end = time.time() + seconds
    side = ["Left", "Right"]
    i = 0
    while time.time() < end:
        g.x("keydown", "space", "keydown", side[i % 2])
        time.sleep(0.8)
        g.x("keyup", side[i % 2])
        if armor_keeper:
            g.x("key", "ctrl+shift+h")
        i += 1
    g.x("keyup", "space")


def label_brightness(g, path, rows_y, x0=8, x1=110):
    """Mean luminance of the text pixels of each menu row, read from the real screenshot
    at the center of each game pixel (same measure whatever the window).
    `rows_y`: center of each row (game pixels)."""
    from PIL import Image

    img = Image.open(path).convert("RGB")
    w = g.state().get("window") or {"origin": [g.ox, g.oy], "pixel": [g.scale, g.scale]}
    (ox, oy), (kx, ky) = w["origin"], w["pixel"]
    out = []
    for y in rows_y:
        vals = []
        for gy in range(y - 6, y + 6):
            for gx in range(x0, x1):
                px = (
                    min(int(ox + (gx + 0.5) * kx), img.width - 1),
                    min(int(oy + (gy + 0.5) * ky), img.height - 1),
                )
                r, gr, b = img.getpixel(px)
                lum = 0.299 * r + 0.587 * gr + 0.114 * b
                if lum > 90:  # text, not the darkened background
                    vals.append(lum)
        out.append(sum(vals) / max(len(vals), 1))
    return out


def journey_pause(g, rep, mode):
    new_game(g, rep, mode, 0, 1)
    start_level(g, rep, mode)
    play(g, 5 if g.light else 20)
    s = g.state()
    rep.check(
        f"play {5 if g.light else 20} s",
        "ship alive, level in progress",
        f"alive {s['level'].get('alive')}, ended {s['level'].get('ended')}",
        s["level"].get("alive") and not s["level"].get("ended"),
    )
    keep_alive(g)
    pause_key(g, rep)
    g.key("Escape")
    g.wait(lambda st: st.get("level", {}).get("pause_menu"), 4, "pause menu")
    mute_check(g, rep, "pause menu")
    g.key("Escape")
    g.wait(lambda st: not st.get("level", {}).get("pause_menu"), 4, "resume")
    if mode == "mouse":
        # The maintainer's case: the mouse drives the ship (captured pointer, relative mode)
        # before Escape; the pause menu is then played with the mouse alone.
        # The preceding keyboard play may leave the ship against the left edge
        # (x = 40, 4 failures on the reference): a step to the left no longer moves
        # anything there. Brought back to the middle with the keyboard, awaited on the game state.
        if (g.state()["level"].get("x") or 0) < 120:
            g.x("keydown", "Right")
            try:
                g.wait(
                    lambda st: (st.get("level", {}).get("x") or 0) >= 120,
                    4,
                    "ship brought back to the middle",
                )
            finally:
                g.x("keyup", "Right")
        x0 = g.settle(0.3)["level"].get("x")
        # Steps of 12 px: below 4 units per tick, the C's (m + 3) / 4
        # (player_movement.c) rounds a movement to the left down to zero.
        for _ in range(15):
            g.x("mousemove_relative", "--", "-12", "0", "sleep", "0.02")
        x1 = g.settle(0.3)["level"].get("x")
        rep.check(
            "mouse in level (15 steps of 12 px to the left)",
            "ship moved to the left",
            f"x {x0} -> {x1}",
            x0 is not None and x1 is not None and x1 < x0,
        )
    g.key("Escape")
    s = g.wait(lambda st: st.get("level", {}).get("pause_menu"), 4, "pause menu")
    pm = s["level"]["pause_menu_state"]
    rep.check(
        "Escape",
        "pause menu open (6 rows)",
        f"{len(pm['labels'])} rows",
        len(pm["labels"]) == 6,
        shot=True,
    )
    # Chosen row visible: its label clearly brighter than the others.
    shot = rep.shots / f"{rep.journey}-pause-sel.png"
    g.screenshot(shot)
    ys = [pm["hits"][str(i)][1] for i in range(6)]
    lum = label_brightness(g, shot, ys)
    others = max(lum[1:])
    rep.check(
        "pause menu: chosen row visible",
        "chosen row label at least 15% brighter than the others",
        f"chosen luminance {lum[0]:.0f}, others max {others:.0f}",
        lum[0] >= others * 1.15,
    )
    rows = [
        ("ig_music_vol", "music_volume"),
        ("ig_sfx_vol", "fx_volume"),
        ("ig_detail", "visual_fx"),
        ("ig_game_speed", "game_speed"),
    ]
    for idx, (label, key) in enumerate(rows):
        choose_menu_row_safe(g, rep, mode, idx, label, pause=True, activate=False)
        seen = [g.state()["settings"][key]]
        presses = {"music_volume": 2, "fx_volume": 2, "visual_fx": 3, "game_speed": 5}[key]
        if g.light:
            presses = 1
        for _ in range(presses):
            v0 = g.state()["settings"][key]
            if mode == "keyboard":
                g.key("Right" if key in ("visual_fx", "game_speed") else "Left")
            else:
                hit = g.state()["level"]["pause_menu_state"]["hits"][str(idx)]
                if key in ("music_volume", "fx_volume"):
                    g.click(130 + 20 * len(seen), hit[1])  # bar column: volume at the point
                else:
                    g.click(*hit)
            v1 = g.settle(0.5)["settings"][key]
            seen.append(v1)
            rep.check(
                f"pause: {label} ({'Right/Left' if mode == 'keyboard' else 'click'})",
                "value changed",
                f"{v0} -> {v1}",
                v1 != v0,
            )
        if key in ("visual_fx", "game_speed") and not g.light:
            rep.check(
                f"pause: {label}, each value",
                f"{presses} values cycled then back to the first",
                f"values seen {seen}",
                len(set(seen)) == presses and seen[-1] == seen[0],
            )
    # Speed: set Turbo (5), resume, check the tick rate.
    choose_menu_row_safe(g, rep, mode, 3, "ig_game_speed", pause=True, activate=False)
    for _ in range(5):
        if g.state()["settings"]["game_speed"] == 5:
            break
        if mode == "keyboard":
            g.key("Right")
        else:
            g.click(*g.state()["level"]["pause_menu_state"]["hits"]["3"])
        g.settle(0.4)
    choose_menu_row_safe(g, rep, mode, 4, "ig_return", pause=True)
    s = g.wait(lambda st: not st.get("level", {}).get("pause_menu"), 4, "resume")
    tps = s["settings"]["physics_ticks_per_second"]
    rep.check(
        "Return to Game with Turbo speed",
        "menu closed, 49 ticks/s (Turbo applied)",
        f"{tps} ticks/s",
        tps == 49,
    )
    # The pause settings are saved (setup.cfg).
    cfg = (g.user_dir() / "setup.cfg").read_text() if (g.user_dir() / "setup.cfg").exists() else ""
    rep.check(
        "pause settings saved",
        "setup.cfg contains game_speed=5",
        "found" if "game_speed=5" in cfg else "missing",
        "game_speed=5" in cfg,
    )
    if not g.light:
        hotkeys(g, rep)
    quit_box(g, rep, mode)


def text_color(g, path, x0, y0, x1, y1):
    """Mean color (r, g, b) of the bright pixels (text) of a zone in game pixels, read from the screenshot."""
    from PIL import Image

    img = Image.open(path).convert("RGB")
    w = g.state().get("window") or {"origin": [g.ox, g.oy], "pixel": [g.scale, g.scale]}
    (ox, oy), (kx, ky) = w["origin"], w["pixel"]
    px = []
    for gy in range(y0, y1):
        for gx in range(x0, x1):
            r, gr, b = img.getpixel(
                (
                    min(int(ox + (gx + 0.5) * kx), img.width - 1),
                    min(int(oy + (gy + 0.5) * ky), img.height - 1),
                )
            )
            if 0.299 * r + 0.587 * gr + 0.114 * b > 120:
                px.append((r, gr, b))
    if not px:
        return (0, 0, 0)
    return tuple(sum(p[i] for p in px) // len(px) for i in range(3))


def pause_key(g, rep):
    """P key (mainint.c:JE_pauseGame): game frozen, "PAUSED" in FONT_SHAPES at (120, 90) with the
    level palette (JE_dString: hue 15 = orange), then P resumes."""
    g.key("p")
    s = g.wait(lambda st: st.get("level", {}).get("paused"), 4, "pause (P)")
    shot = rep.shots / f"{rep.journey}-paused.png"
    g.screenshot(shot)
    r, gr, b = text_color(g, shot, 120, 90, 230, 106)
    rep.check(
        'P key: "PAUSED"',
        "orange text as in the C original (red > blue + 60)",
        f"mean color ({r}, {gr}, {b})",
        s["level"].get("paused") and r > b + 60,
        shot=True,
    )
    g.key("p")
    g.wait(lambda st: not st.get("level", {}).get("paused"), 4, "resume (P)")


def mute_check(g, rep, where):
    """Sound mute key (F8): mutes music and effects, restores exactly,
    without touching the volumes or the "disabled" settings (divergence no. 57)."""
    s0 = g.state()["settings"]
    g.key("F8")
    s1 = g.settle(0.5)["settings"]
    rep.check(
        f"{where}: F8",
        "sound muted (music and effects), volumes unchanged",
        f"muted {s1['sound_muted']}, music muted {s1['music_is_muted']}, effects enabled {s1['sfx_is_enabled']}, "
        f"volumes {s1['music_volume']}/{s1['fx_volume']}",
        s1["sound_muted"] is True
        and s1["music_is_muted"] is True
        and s1["sfx_is_enabled"] is False
        and s1["music_volume"] == s0["music_volume"]
        and s1["fx_volume"] == s0["fx_volume"]
        and s1["music_disabled"] == s0["music_disabled"]
        and s1["samples_disabled"] == s0["samples_disabled"],
    )
    cfg = (g.user_dir() / "setup.cfg").read_text() if (g.user_dir() / "setup.cfg").exists() else ""
    rep.check(
        f"{where}: F8 saved",
        "setup.cfg contains sound_muted=true",
        "found" if "sound_muted=true" in cfg else "missing",
        "sound_muted=true" in cfg,
    )
    g.key("F8")
    s2 = g.settle(0.5)["settings"]
    rep.check(
        f"{where}: F8 again",
        "sound restored exactly as before",
        f"muted {s2['sound_muted']}, music muted {s2['music_is_muted']}, effects enabled {s2['sfx_is_enabled']}, "
        f"music dB {s0['music_player_db']} -> {s2['music_player_db']}",
        s2["sound_muted"] is False
        and s2["music_is_muted"] == s0["music_is_muted"]
        and s2["sfx_is_enabled"] == s0["sfx_is_enabled"]
        and s2["music_volume"] == s0["music_volume"]
        and abs(s2["music_player_db"] - s0["music_player_db"]) < 0.01,
    )


def keep_alive(g):
    """--devmode: Ctrl+Shift+H restores the armor (dev_console.c), the ship does not die during the checks.
    Never in pause: any key there resumes the game."""
    s = g.state()
    if (
        "--devmode" in g.extra_args
        and s.get("scene_class") == "level.gd"
        and not s.get("level", {}).get("paused")
        and not s.get("level", {}).get("pause_menu")
    ):
        g.x("key", "ctrl+shift+h")
        time.sleep(0.15)


def level_key(g, key):
    """Level key: armor restored just before, the level must still be running."""
    keep_alive(g)
    if g.state().get("scene_class") != "level.gd":
        raise JourneyFailure(f"level left before {key} (ship destroyed?)")
    g.key(key)


def hotkeys(g, rep):
    ud = g.user_dir()
    keep_alive(g)
    mute_check(g, rep, "level")
    keep_alive(g)
    # F2 : pause.
    level_key(g, "F2")
    s = g.settle(0.6)
    rep.check("F2", "game paused", f"pause {s['level'].get('paused')}", s["level"].get("paused"))
    level_key(g, "F2")
    s = g.settle(0.6)
    rep.check(
        "F2 again",
        "resumed",
        f"pause {s['level'].get('paused')}",
        not s["level"].get("paused"),
    )
    for k, key, what in [
        ("F3", "achievements", "achievements"),
        ("F4", "stats", "statistics"),
        ("F6", "dps", "DPS counter"),
    ]:
        keep_alive(g)
        level_key(g, k)
        s = g.settle(0.6)
        on = s["level"].get("overlays", {}).get(key)
        rep.check(k, f"{what} shown", f"{on}", on is True, shot=True)
        level_key(g, k)
        s = g.settle(0.6)
        rep.check(
            f"{k} again",
            f"{what} hidden",
            f"{s['level'].get('overlays', {}).get(key)}",
            s["level"].get("overlays", {}).get(key) is False,
        )
    v0 = g.state()["settings"]["damage_numbers_enabled"]
    level_key(g, "F7")
    v1 = g.settle(0.6)["settings"]["damage_numbers_enabled"]
    rep.check("F7", "damage numbers toggled", f"{v0} -> {v1}", v1 != v0)
    level_key(g, "F7")
    quick = ud / "saves" / "slot_11.json"
    t0 = quick.stat().st_mtime if quick.exists() else 0
    level_key(g, "F5")
    time.sleep(1)
    ok = quick.exists() and quick.stat().st_mtime > t0
    saved = json.loads(quick.read_text()) if quick.exists() else {}
    rep.check(
        "F5",
        "quick save written (slot 11, QUICK SAVE)",
        f"{saved.get('name')} {saved.get('cash')}",
        ok,
    )
    level_key(g, "F9")
    s = g.settle(0.6)
    rep.check(
        "F9",
        "quick load confirmation prompt",
        f"{s['level'].get('quick_load_pending')}",
        s["level"].get("quick_load_pending") is True,
        shot=True,
    )
    g.key("n")
    s = g.settle(0.6)
    rep.check(
        "F9 then N",
        "load cancelled, level continues",
        f"pending {s['level'].get('quick_load_pending')}, scene {s['scene_class']}",
        not s["level"].get("quick_load_pending") and s["scene_class"] == "level.gd",
    )
    # F10 (replay.c): outside recording, arms the recording of the next level.
    keep_alive(g)
    level_key(g, "F10")
    s = g.settle(0.8)
    rep.check(
        "F10",
        "replay recording armed for the next level",
        f"armed {s['level'].get('recording_armed')}",
        s["level"].get("recording_armed") is True,
    )
    level_key(g, "F10")
    s = g.settle(0.8)
    rep.check(
        "F10 again",
        "recording disarmed",
        f"armed {s['level'].get('recording_armed')}",
        s["level"].get("recording_armed") is False,
    )
    gm0 = g.state()["settings"]["display_gamma"]
    level_key(g, "F11")
    gm1 = g.settle(0.6)["settings"]["display_gamma"]
    rep.check("F11", "gamma changed", f"{gm0} -> {gm1}", gm1 != gm0)
    for _ in range(3):
        level_key(g, "F11")
    shots0 = len(list(ud.glob("screenshot_*.bmp")))
    level_key(g, "F12")
    time.sleep(1)
    shots1 = len(list(ud.glob("screenshot_*.bmp")))
    rep.check("F12", "screenshot written", f"{shots0} -> {shots1} file(s)", shots1 == shots0 + 1)


def open_quit_box(g, rep, mode):
    keep_alive(g)
    g.key("Escape")
    g.wait(lambda st: st.get("level", {}).get("pause_menu"), 4, "pause menu")
    choose_menu_row_safe(g, rep, mode, 5, "ig_quit", pause=True)
    time.sleep(1)


def quit_box(g, rep, mode):
    open_quit_box(g, rep, mode)
    s = g.state()
    modal = bool(s.get("level", {}).get("pause_menu"))
    rep.check("Quit", '"Quit?" box open', "open" if modal else "closed", modal, shot=True)
    # No: "No" is preselected.
    if mode == "keyboard":
        g.key("Return")
    else:
        g.click(194, 136)
    s = g.settle(1.2)
    rep.check(
        "Quit box: No",
        "back to the level",
        f"scene {s['scene_class']}, pause {s.get('level', {}).get('pause_menu')}",
        s["scene_class"] == "level.gd" and not s.get("level", {}).get("pause_menu"),
    )
    open_quit_box(g, rep, mode)
    if mode == "keyboard":
        g.key("Left")
        g.key("Return")
    else:
        g.click(99, 136)
    try:
        s = g.wait(lambda st: st.get("scene_class") == "shop.gd", 15, "back to the shop")
        rep.check(
            "Quit box: Yes",
            "level abandoned, back to the shop",
            f"scene {s['scene_class']}",
            True,
            shot=True,
        )
    except JourneyFailure as e:
        rep.fail("Quit box: Yes", "level abandoned, back to the shop", str(e))


def journey_finish_level(g, rep, mode):
    new_game(g, rep, mode, 0, 1)
    # A purchase first: the equipment to keep is not the starting one.
    buy_in_category(g, rep, mode, 3)
    back_to_shop_main(g)
    before = list(shop_state(g)["committed_loadout"])
    start_level(g, rep, mode)
    play(g, 4)
    cash_level = g.state()["level"]["cash"]
    g.key("ctrl+shift+n")
    try:
        g.wait(lambda st: st.get("scene_class") not in ("level.gd",), 30, "level end")
    except JourneyFailure as e:
        rep.fail("Ctrl+Shift+N (level end, --devmode)", "level end screen", str(e))
        return
    s = g.state()
    rep.check("Ctrl+Shift+N", "level end screen", f"scene {s['scene_class']}", True, shot=True)
    for _ in range(30):
        s = g.state()
        if s.get("scene_class") == "shop.gd" and s.get("screen", {}).get("idle"):
            break
        g.key("Return", pause=0.8)
    s = g.state()
    if s.get("scene_class") != "shop.gd":
        rep.fail(
            "debriefing then shop",
            "shop of the next section",
            f"scene {s.get('scene_class')}",
        )
        return
    after = s["shop"]["committed_loadout"]
    rep.check(
        "shop after the level",
        "same equipment as before the level",
        f"before {before[:10]}, after {after[:10]}",
        after[:10] == before[:10],
        shot=True,
    )
    rep.check(
        "shop after the level",
        f"cash ≥ end-of-level cash {cash_level}",
        f"{after[10]}",
        after[10] >= cash_level,
    )


def journey_save_load(g, rep, mode):
    new_game(g, rep, mode, 0, 1)
    # Full batch: a purchase first, the save must keep non-standard equipment.
    if not g.light:
        buy_in_category(g, rep, mode, 3)
    save_and_reload(g, rep, mode)


def save_and_reload(g, rep, mode):
    """Options > Save Game (slot 2), Quit Game, Load Game: equipment and money read back identically."""
    back_to_shop_main(g)
    saved_lo = list(shop_state(g)["committed_loadout"])
    shop_goto_row(g, rep, mode, 5, "Options")
    g.wait(
        lambda st: st.get("scene_class") == "save_menu.gd" and st.get("menu", {}).get("labels"),
        6,
        "Options menu",
    )
    choose_menu_row_safe(g, rep, mode, 1, "Save Game")
    g.wait(lambda st: st.get("menu", {}).get("depth") == 2, 4, "slot list")
    choose_menu_row_safe(g, rep, mode, 1, "slot 2")
    g.wait(lambda st: st.get("screen", {}).get("_typing"), 4, "name entry")
    g.x("type", "--delay", "90", "JOURNEY")
    time.sleep(0.4)
    g.key("Return")
    time.sleep(0.8)
    g.key("Return")  # confirmation (JE_saveRequest)
    path = g.user_dir() / "saves" / "slot_02.json"
    for _ in range(20):
        if path.exists():
            break
        time.sleep(0.25)
    data = json.loads(path.read_text()) if path.exists() else {}
    rep.check(
        "Save Game, slot 2, name JOURNEY",
        "slot_02.json file written with the name and the cash",
        f"name {data.get('name')}, cash {data.get('cash')}",
        data.get("name") == "JOURNEY" and data.get("cash") == saved_lo[10],
        shot=True,
    )
    for _ in range(4):
        if g.state().get("scene_class") == "shop.gd":
            break
        g.key("Escape", pause=0.7)
    back_to_shop_main(g)
    shop_goto_row(g, rep, mode, 7, "Quit Game")
    time.sleep(0.8)
    g.key("Left")
    g.key("Return")
    try:
        g.wait(lambda st: st.get("scene_class") == "title.gd", 10, "title")
        rep.check("Quit Game : Yes", "back to the title", "title", True)
    except JourneyFailure as e:
        rep.fail("Quit Game : Yes", "back to the title", str(e))
        return
    g.wait(lambda s: s.get("menu", {}).get("labels"), 15, "title menu")
    choose_menu_row_safe(g, rep, mode, 1, "Load Game")
    g.wait(
        lambda st: st.get("scene_class") == "load_screen.gd" and st.get("menu", {}).get("hits"),
        8,
        "Load Game screen",
    )
    choose_menu_row_safe(g, rep, mode, 1, "slot 2")
    try:
        s = g.wait(
            lambda st: st.get("scene_class") == "shop.gd" and st.get("shop", {}).get("open"),
            15,
            "loaded shop",
        )
    except JourneyFailure as e:
        rep.fail("Load Game, slot 2", "shop of the loaded game", str(e))
        return
    lo = s["shop"]["committed_loadout"]
    rep.check(
        "Load Game, slot 2",
        f"same cash and equipment {saved_lo}",
        f"{lo}",
        lo == saved_lo,
        shot=True,
    )
    rep.check(
        "Load Game, slot 2",
        "same level (shop section)",
        f"section {s['game']['shop_section']}",
        s["game"]["shop_section"] == data.get("section", s["game"]["shop_section"]),
    )


SETUP_SUBMENUS = ["setup_graphics", "setup_sound", "setup_gameplay"]


def journey_title(g, rep, mode):
    wait_title_menu(g)
    mute_check(g, rep, "title")
    labels = g.state()["menu"]["labels"]
    expect = {
        "Load Game": "load_screen.gd",
        "High Scores": "high_scores.gd",
        "Instructions": "instructions.gd",
        "Setup": "setup.gd",
        "Achievements": "achievements_screen.gd",
        "Replays": "replays_screen.gd",
    }
    if g.light:
        expect = {"Setup": "setup.gd"}  # Load Game: already played by the save journey
    for idx, label in enumerate(labels):
        if label not in expect:
            continue
        choose_menu_row_safe(g, rep, mode, idx, label)
        try:
            s = g.wait(lambda st, label=label: st.get("scene_class") == expect[label], 8, label)
            rep.check(f"title: {label}", f"screen {expect[label]}", s["scene_class"], True, shot=True)
        except JourneyFailure as e:
            rep.fail(f"title: {label}", f"screen {expect[label]}", str(e))
            g.kill()
            g.launch()
            continue
        if label == "Setup":
            setup_submenus(g, rep, mode, light=g.light)
        back_to_title(g, rep, label, mode)
    # Setting kept after restarting the game.
    before = g.state()["settings"]
    quit_title(g, rep, mode)
    g.launch()
    after = g.wait(lambda s: s.get("scene_class") == "title.gd", 25, "title after restart")["settings"]
    diffs = [
        k
        for k in (
            "explosion_style",
            "music_engine",
            "damage_numbers_enabled",
            "display_crt_mode",
            "visual_fx",
        )
        if before.get(k) != after.get(k)
    ]
    rep.check(
        "game restart",
        "Setup settings kept",
        "identical" if not diffs else f"different: {diffs}",
        not diffs,
    )


def back_to_title(g, rep, label, mode):
    for _ in range(4):
        if g.state().get("scene_class") == "title.gd":
            break
        if mode == "keyboard":
            g.key("Escape", pause=1.0)
        else:
            g.x("click", "3")
            time.sleep(1.0)
    ok = g.state().get("scene_class") == "title.gd"
    if not ok:
        g.key("Escape", pause=1.0)
        ok = g.state().get("scene_class") == "title.gd"
    rep.check(
        f"{label}: {'Escape' if mode == 'keyboard' else 'right click'}",
        "back to the title",
        g.state().get("scene_class"),
        ok,
    )
    if not ok:
        g.kill()
        g.launch()
    g.wait(lambda s: s.get("menu", {}).get("labels"), 15, "title menu")


def setup_submenus(g, rep, mode, light=False):
    g.wait(lambda st: st.get("menu", {}).get("labels"), 6, "Setup menu")
    labels = g.state()["menu"]["labels"]
    # Scaler (core too): the window changes size, the image must follow.
    if "setup_graphics" in labels:
        choose_menu_row_safe(g, rep, mode, labels.index("setup_graphics"), "setup_graphics")
        g.wait(lambda st: st.get("menu", {}).get("depth") == 2, 4, "setup_graphics")
        scaler_window(g, rep, mode)
        g.key("Escape")
        g.wait(lambda st: st.get("menu", {}).get("depth") == 1, 4, "back to the Setup menu")
    for name in SETUP_SUBMENUS[2:] if light else SETUP_SUBMENUS:
        if name not in labels:
            rep.fail(f"Setup: {name}", "row present", f"rows {labels}")
            continue
        idx = labels.index(name)
        choose_menu_row_safe(g, rep, mode, idx, name)
        s = g.wait(lambda st: st.get("menu", {}).get("depth") == 2, 4, name)
        sub = s["menu"]
        before = dict(s["settings"])
        # First row with a value (PICKER): Right / click changes the setting.
        changed = None
        for row in range(len(sub["labels"]) - 1):
            choose_menu_row(g, rep, mode, row, sub["labels"][row], activate=False)
            if mode == "keyboard":
                g.key("Right")
            else:
                g.x("click", "1")
                time.sleep(0.4)
            s = g.settle(0.6)
            if s.get("menu", {}).get("depth", 0) > 2 or s.get("menu", {}).get("popup"):
                g.key("Escape")
                s = g.settle(0.6)
                if s.get("menu", {}).get("popup"):
                    g.key("Escape")
                    s = g.settle(0.6)
                continue
            diff = {
                k: (before[k], s["settings"][k])
                for k in before
                if before[k] != s["settings"].get(k) and k not in ("physics_ticks_per_second", "music_user_volume")
            }
            if diff:
                changed = (sub["labels"][row], diff)
                break
        if changed is None:
            rep.fail(f"Setup > {name}", "a value changed by Right/click", "no setting changed")
        else:
            rep.check(f"Setup > {name} > {changed[0]}", "setting changed", f"{changed[1]}", True)
        g.key("Escape")
        g.wait(lambda st: st.get("menu", {}).get("depth") == 1, 4, "back to the Setup menu")
        if changed:
            choose_menu_row_safe(g, rep, mode, idx, name)
            s = g.wait(lambda st: st.get("menu", {}).get("depth") == 2, 4, name)
            kept = all(s["settings"].get(k) == v[1] for k, v in changed[1].items())
            rep.check(
                f"Setup > {name} reopened",
                "setting kept",
                f"{ {k: s['settings'].get(k) for k in changed[1]} }",
                kept,
            )
            g.key("Escape")
            g.wait(lambda st: st.get("menu", {}).get("depth") == 1, 4, "back to the Setup menu")
    for name, _scene in (
        []
        if light
        else [
            ("setup_jukebox", "jukebox.gd"),
            ("setup_mods", "mods_menu.gd"),
            ("setup_keybindings", "controls_options.gd"),
        ]
    ):
        if name not in labels:
            continue
        choose_menu_row_safe(g, rep, mode, labels.index(name), name)
        try:
            s = g.wait(
                lambda st: st.get("scene_class") != "setup.gd" or st.get("menu", {}).get("depth", 1) > 1,
                6,
                name,
            )
            rep.check(f"Setup > {name}", "screen open", s.get("scene_class"), True, shot=True)
        except JourneyFailure as e:
            rep.fail(f"Setup > {name}", "screen open", str(e))
        for _ in range(3):
            if g.state().get("scene_class") == "setup.gd" and g.state().get("menu", {}).get("depth") == 1:
                break
            g.key("Escape", pause=1.0)
        ok = g.state().get("scene_class") == "setup.gd"
        rep.check(f"Setup > {name}: Escape", "back to the Setup menu", g.state().get("scene_class"), ok)
        if not ok:
            raise JourneyFailure(f"{name}: cannot return to Setup")


def pick_in_popup(g, rep, mode, row, what, target):
    """Opens the dropdown list of row `row` and chooses `target` in it (keyboard or mouse)."""
    choose_menu_row(g, rep, mode, row, what)
    s = g.wait(lambda st: st.get("menu", {}).get("popup_hits", {}).get("hits"), 4, f"list {what}")
    if mode == "keyboard":
        steps = target - s["menu"]["popup_hits"]["sel"]
        if steps:
            g.key(*(["Down"] * steps if steps > 0 else ["Up"] * -steps))
        g.key("Return")
    else:
        x, y = s["menu"]["popup_hits"]["hits"][str(target)]
        g.click(x, y)


def scaler_window(g, rep, mode):
    """Setup > Graphics > Scaler chosen in the list: the window takes K x 320x200 and
    the whole image fits in it (video.c:init_scaler). Before the fix the window shrank
    but Godot kept the old content size: a corner of the image, enlarged."""
    labels = g.state()["menu"]["labels"]
    if "gfx_scaler" not in labels:
        rep.fail("Setup > Graphics > Scaler", "row present", f"rows {labels}")
        return
    row = labels.index("gfx_scaler")
    before = g.state()["settings"].get("display_scaler")
    target = 1 if before != 1 else 2  # 2x (or 3x): known integer factor
    factor = {1: 2, 2: 3}[target]
    pick_in_popup(g, rep, mode, row, "gfx_scaler", target)
    want = [320 * factor, 200 * factor]
    try:
        s = g.wait(
            lambda st: (
                st.get("window", {}).get("size") == want
                and abs(st["window"]["pixel"][0] - factor) < 0.01
                and st["window"]["origin"] == [0.0, 0.0]
            ),
            6,
            "window and image at the scaler factor",
        )
        ok = True
    except JourneyFailure:
        s, ok = g.state(), False
    w = s.get("window", {})
    rep.check(
        f"Setup > Graphics > Scaler {factor}x",
        f"window {want}, whole image (pixel {factor}, origin 0)",
        f"window {w.get('size')}, pixel {w.get('pixel')}, origin {w.get('origin')}",
        ok,
        shot=True,
    )
    # Previous scaler, then the journey's window.
    g.mouse = (
        int(w.get("size", [g.width, g.height])[0]) // 2,
        int(w.get("size", [g.width, g.height])[1]) // 2,
    )
    pick_in_popup(g, rep, "keyboard", row, "gfx_scaler", before)
    g.settle(1.0)
    for _ in range(10):
        g.x("windowsize", g.wid, str(g.width), str(g.height))
        g.x("windowmove", g.wid, "0", "0")
        time.sleep(0.3)
        if g.state().get("window", {}).get("size") == [g.width, g.height]:
            break
    g.mouse = (g.width // 2, g.height // 2)


def quit_title(g, rep, mode):
    g.wait(
        lambda s: s.get("scene_class") == "title.gd" and s.get("menu", {}).get("labels"),
        10,
        "title",
    )
    labels = g.state()["menu"]["labels"]
    choose_menu_row_safe(g, rep, mode, labels.index("Quit"), "Quit")
    time.sleep(1)
    g.key("Left")
    g.key("Return")
    rc = g.quit(10)
    rep.check("title: Quit then Yes", "the game closes (code 0)", f"code {rc}", rc == 0)
    if rc is None:
        g.kill()


def journey_arcade(g, rep, mode, two_players):
    kind = 4 if two_players else 1
    new_game(g, rep, mode, kind, 1, expect_shop=False)
    try:
        s = g.wait(
            lambda st: st.get("scene_class") == "shop.gd" and st.get("screen", {}).get("_arcade_menu"),
            20,
            "Game Menu arcade",
        )
    except JourneyFailure as e:
        rep.fail(f"arcade {'2P' if two_players else '1P'}", "arcade Game Menu", str(e))
        return
    rep.check(
        f"arcade {'2P' if two_players else '1P'}",
        "arcade Game Menu",
        f"menu {s['screen']['_arcade_menu']}, selection {s['screen']['_arcade_sel']}",
        True,
        shot=True,
    )
    # Navigation: each row reached with the keyboard / the mouse.
    am = s["screen"]["_arcade_menu"]
    # ARCADE_MENU_INT: header then rows 2..n+1 (1P: Start, Options, Quit).
    n = {9: 5, 10: 3, 13: 4}.get(am, 3)
    hits = s.get("arcade_hits", {})
    seen = set()
    for r in range(2, n + 2):
        if mode == "keyboard":
            g.key("Down")
        else:
            if str(r) not in hits:
                continue
            g.move(hits[str(r)][0] + 1, hits[str(r)][1])
            g.move(*hits[str(r)])
        seen.add(g.settle(0.4).get("screen", {}).get("_arcade_sel"))
    rows = sorted(x for x in seen if x is not None)
    rep.check(
        "arcade: browse the Game Menu",
        f"rows 2 to {n + 1} reached",
        f"rows {rows}",
        rows == list(range(2, n + 2)),
    )
    # Options then back.
    opt_row = 5 if am == 9 else 3
    arcade_goto(g, mode, opt_row)
    try:
        g.wait(lambda st: st.get("scene_class") == "save_menu.gd", 6, "Options")
        rep.check("arcade: Options", "Options menu", "open", True)
        for _ in range(3):
            if g.state().get("scene_class") == "shop.gd":
                break
            g.key("Escape", pause=1.0)
        rep.check(
            "arcade: Options then Escape",
            "back to the Game Menu",
            g.state().get("scene_class"),
            g.state().get("scene_class") == "shop.gd",
        )
    except JourneyFailure as e:
        rep.fail("arcade: Options", "Options menu", str(e))
    arcade_goto(g, mode, 2)
    try:
        s = g.wait(
            lambda st: st.get("scene_class") == "level.gd" and "armor" in st.get("level", {}),
            30,
            "arcade level",
        )
        rep.check("arcade: Start Level", "level started", s["scene_class"], True)
    except JourneyFailure as e:
        rep.fail("arcade: Start Level", "level started", str(e))
        return
    play(g, 5, armor_keeper=False)
    open_quit_box(g, rep, mode)
    g.key("Left")
    g.key("Return")
    try:
        s = g.wait(
            lambda st: st.get("scene_class") in ("shop.gd", "title.gd"),
            15,
            "arcade level exit",
        )
        rep.check("arcade: Quit > Yes", "level exit", s["scene_class"], True, shot=True)
    except JourneyFailure as e:
        rep.fail("arcade: Quit > Yes", "level exit", str(e))


def arcade_goto(g, mode, row):
    for _ in range(8):
        if g.state().get("screen", {}).get("_arcade_sel") == row:
            break
        if mode == "keyboard":
            g.key("Down", pause=0.3)
        else:
            hit = g.state().get("arcade_hits", {}).get(str(row))
            if hit:
                g.move(hit[0] + 1, hit[1])
                g.move(*hit)
        g.settle(0.3)
    if mode == "keyboard":
        g.key("Return")
    else:
        g.x("click", "1")
        time.sleep(0.4)
        if g.state().get("scene_class") == "shop.gd" and g.state().get("screen", {}).get("_arcade_sel") == row:
            g.x("click", "1")
    time.sleep(0.6)


def journey_endless(g, rep, mode):
    new_game(g, rep, mode, 2, 1, expect_shop=False)
    for _ in range(10):
        s = g.state()
        if s.get("scene_class") == "level.gd" and s.get("endless", {}).get("active"):
            break
        g.key("Return", pause=1.0)
    s = g.state()
    if not (s.get("scene_class") == "level.gd" and s.get("endless", {}).get("active")):
        rep.fail(
            "Endless 1P",
            "Endless level started",
            f"scene {s.get('scene_class')}, endless {s.get('endless')}",
        )
        return
    rep.check("Endless 1P", "Endless level started", f"lives {s['endless'].get('lives')}", True, shot=True)
    play(g, 25, armor_keeper=False)
    s = g.state()
    final_score = int(s.get("endless", {}).get("score") or 0)
    rep.check(
        "Endless: 25 s of play",
        "Endless game in progress",
        f"score {final_score}",
        s.get("endless", {}).get("active", False),
    )
    open_quit_box(g, rep, mode)
    g.key("Left")
    g.key("Return")
    try:
        s = g.wait(lambda st: st.get("scene_class") != "level.gd", 15, "Endless game end")
        rep.check("Endless: Quit > Yes", "end / ranking screen", s["scene_class"], True, shot=True)
    except JourneyFailure as e:
        rep.fail("Endless: Quit > Yes", "end / ranking screen", str(e))
        return
    # Name (Escape = "PLAYER"), summary, ranking (Escape = title; Enter restarts a game there).
    final_score = int(g.state().get("endless", {}).get("final_score") or 0)
    for _ in range(12):
        if g.state().get("scene_class") == "title.gd":
            break
        g.key("Escape", pause=1.5)
    board = g.user_dir() / "endless_leaderboard.json"
    entries = json.loads(board.read_text()) if board.exists() else []
    # endless.c:endless_qualifies: non-zero final score (waves + money) -> recorded under "PLAYER".
    expect_entry = final_score > 0
    found = [e for e in entries if int(e.get("score", 0)) == final_score and e.get("name") == "PLAYER"]
    rep.check(
        "Endless: name (Escape), summary, ranking, Escape",
        f"title reached; final score {final_score} recorded (PLAYER)",
        f"scene {g.state().get('scene_class')}, ranking {entries}",
        g.state().get("scene_class") == "title.gd" and bool(found) == expect_entry,
    )


WINDOW_RECT = [150, 90, 1100, 700]


def remembered_window_profile(home):
    """The maintainer's profile as seen on 2026-10-10: remembered window (divergence 58), start screen
    "window", HD sprites. Written before the first launch."""
    user = home / USER_DIR
    user.mkdir(parents=True, exist_ok=True)
    x, y, w, h = WINDOW_RECT
    (user / "setup.cfg").write_text(
        f"[setup]\n\ndisplay=0\nwindow_rect=[{x}, {y}, {w}, {h}]\n\n[format]\n\ndisplay_default=2\n"
    )
    (user / "display.cfg").write_text("[display]\n\nart_mode=2\ncrt_mode=0\nscaler=0\n")


def journey_remembered_window(g, rep, mode):
    """Launch with a remembered window: the window returns to its size and the whole image
    fits in it, at the integer factor it allows. Before the fix, the window did resume
    1100x700 but the image stayed laid out for 960x600 (or 320x200 pixel for pixel in
    full screen): small image in a corner."""
    x, y, w, h = WINDOW_RECT
    k = min(w // 320, h // 200)
    want_origin = [float((w - 320 * k) // 2), float((h - 200 * k) // 2)]
    try:
        s = g.wait(
            lambda st: st.get("window", {}).get("size") == [w, h] and abs(st["window"]["pixel"][0] - k) < 0.01,
            10,
            "remembered window and fitted image",
        )
        ok = True
    except JourneyFailure:
        s, ok = g.state(), False
    win = s.get("window", {})
    rep.check(
        "launch, remembered window",
        f"window {w}x{h}, image at factor {k}, origin {want_origin}",
        f"window {win.get('size')}, pixel {win.get('pixel')}, origin {win.get('origin')}",
        ok,
        shot=True,
    )
    # The title rows are picked with the mouse in this window.
    wait_title_menu(g)
    choose_menu_row(g, rep, "mouse", 4, "Setup", activate=False)


def journey_maintainer_profile(g, rep, mode):
    """The maintainer's real profile: his LAST LEVEL save loads and the shop buys."""
    wait_title_menu(g)
    slots = sorted(p.name for p in (g.user_dir() / "saves").glob("slot_*.json"))
    rep.check("Maintainer's profile", "his saves are read", f"{slots}", bool(slots))
    choose_menu_row_safe(g, rep, mode, 1, "Load Game")
    g.wait(
        lambda st: st.get("scene_class") == "load_screen.gd" and st.get("menu", {}).get("hits"),
        8,
        "Load Game",
    )
    saved = json.loads((g.user_dir() / "saves" / "slot_11.json").read_text())
    choose_menu_row_safe(g, rep, mode, 10, "LAST LEVEL (slot 11)")
    try:
        s = g.wait(
            lambda st: st.get("scene_class") == "shop.gd" and st.get("shop", {}).get("open"),
            15,
            "loaded shop",
        )
    except JourneyFailure as e:
        rep.fail("Load Game: LAST LEVEL", "shop of his game", str(e))
        return
    rep.check(
        "Load Game: LAST LEVEL",
        f"cash {saved['cash']}",
        f"cash {s['game']['cash']}",
        s["game"]["cash"] == saved["cash"],
        shot=True,
    )
    for cat in (3,) if g.light else (3, 6):
        buy_in_category(g, rep, mode, cat)


JOURNEYS = {
    # name: (function, core?, needs --devmode, the maintainer's profile)
    "shop": (
        lambda g, r, m: journey_shop(g, r, m, 1 if m == "keyboard" else 2),
        True,
        False,
        False,
    ),
    "shop-easy": (lambda g, r, m: journey_shop(g, r, m, 0), False, False, False),
    "shop-mods": (journey_shop_mods, False, True, False),
    "pause": (journey_pause, True, True, False),
    "finish-level": (journey_finish_level, False, True, False),
    "save-load": (journey_save_load, True, False, False),
    "title": (journey_title, True, False, False),
    "arcade-1p": (lambda g, r, m: journey_arcade(g, r, m, False), False, False, False),
    "arcade-2p": (lambda g, r, m: journey_arcade(g, r, m, True), False, False, False),
    "endless": (journey_endless, False, False, False),
    "maintainer-profile": (journey_maintainer_profile, True, False, True),
    "remembered-window": (journey_remembered_window, True, False, False),
}


def run_journey(a, out, report, name, fn, devmode, uses_profile, mode, size):
    tag = f"{name}-{mode}-{size[0]}x{size[1]}"
    home = out / "homes" / tag
    shutil.rmtree(home, ignore_errors=True)
    if uses_profile:
        shutil.copytree(a.profile, home / USER_DIR)
    home.mkdir(parents=True, exist_ok=True)
    extra = ["--devmode"] if devmode else []
    # The maintainer's real setting (display.cfg art_mode=2): pause menu with the mouse in HD sprites,
    # where the game viewport is 1280x800 (positions badly converted before the fix).
    if name == "pause" and mode == "mouse":
        extra.append("--hd-sprites")
    # Core: intro logos passed (--no-intro, existing option) except for the title journey.
    if a.core and name != "title":
        extra.append("--no-intro")
    if name == "shop-mods":
        extra.append(f"--mods={','.join(SHIPPED_MODS)}")
    if name == "remembered-window":
        remembered_window_profile(home)
    g = Game(a.package, out, size, home, extra_args=extra, tag=tag)
    g.light = a.core
    g.free_window = name == "remembered-window"
    rep = report.journey(tag, g)
    t0 = time.time()
    print(f"== {tag}: start", flush=True)
    try:
        g.start()
        fn(g, rep, mode)
    except JourneyFailure as e:
        rep.fail("journey interrupted", "complete journey", str(e))
    except Exception as e:  # noqa: BLE001 - a driver defect is a failure, not a batch stop
        rep.fail("driver error", "complete journey", f"{type(e).__name__}: {e}")
    finally:
        if g.alive():
            g.screenshot(rep.shots / f"{tag}-fin.png")
        g.close()
        log = g.log_path.read_text(errors="replace") if g.log_path.exists() else ""
        errs = [ln for ln in log.splitlines() if "SCRIPT ERROR" in ln]
        rep.check("Godot log", "no SCRIPT ERROR", f"{len(errs)} : {errs[:2]}", not errs)
        print(f"== {tag}: end ({time.time() - t0:.0f} s)", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--package", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--core", action="store_true")
    ap.add_argument("--only", default="")
    ap.add_argument("--modes", default="keyboard,mouse")
    ap.add_argument("--sizes", default="1280x684,1920x1080")
    ap.add_argument("--profile", default="")
    ap.add_argument("--rev", default="?")
    ap.add_argument("--jobs", type=int, default=2, help="simultaneous games (2 at most)")
    a = ap.parse_args()
    stop_games_on_signal()
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    report = Report(out)
    started = time.time()
    sizes = [tuple(int(v) for v in s.split("x")) for s in a.sizes.split(",") if s]
    if a.core and a.sizes == ap.get_default("sizes"):
        sizes = sizes[:1]  # core before push: 1280x684; the full batch also plays 1920x1080
    only = [x for x in a.only.split(",") if x]
    work = []
    for si, size in enumerate(sizes):
        for name, (fn, core, devmode, uses_profile) in JOURNEYS.items():
            if only and name not in only:
                continue
            if a.core and not core:
                continue
            # Large window: the core only (display size, not game logic).
            if si > 0 and not core:
                continue
            if uses_profile and not a.profile:
                continue
            for mode in a.modes.split(","):
                # Core: the maintainer's real profile with the keyboard only (his mouse is checked by the full batch).
                if a.core and uses_profile and mode != a.modes.split(",")[0]:
                    continue
                work.append((name, fn, devmode, uses_profile, mode, size))
    # Longest first: the two simultaneous games finish together.
    longest_first = [
        "pause",
        "title",
        "finish-level",
        "endless",
        "save-load",
        "shop",
        "shop-easy",
    ]
    work.sort(key=lambda w: longest_first.index(w[0]) if w[0] in longest_first else len(longest_first))
    with ThreadPoolExecutor(max_workers=max(1, min(a.jobs, 2))) as pool:
        for f in [pool.submit(run_journey, a, out, report, *w) for w in work]:
            f.result()
    dur = time.time() - started
    meta = {
        "rev": a.rev,
        "package": a.package,
        "duration": f"{int(dur // 60)} min {int(dur % 60)} s",
    }
    report.write(out / "report.md", meta)
    rows = report.rows
    fails = sum(1 for r in rows if r["status"] == "FAIL")
    print(f"== {len(rows)} steps, {fails} FAIL, {meta['duration']} — {out / 'report.md'}")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
