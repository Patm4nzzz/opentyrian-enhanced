#!/usr/bin/env python3
"""Driver for the player journeys (rewrite/scripts/player-journeys.sh).

Plays the exported Linux package under Xvfb with real X inputs (xdotool:
keys, mouse moved in small steps, clicks) and checks after each
action the EFFECT in the game state, read back from the file written by the
JourneyState autoload (--journey-state=<file>) or from the files the game
writes (saves, setup.cfg). An image is never enough: each
step compares an expected effect to an observed effect.

Usage: player_journeys.py --package DIR --out DIR [--core] [--only NAME,...]
        [--size 1280x684,1920x1080] [--profile C_PROFILE_DIR]
"""

import json
import os
import signal
import subprocess
import threading
import time
from pathlib import Path

EXE = "OpenTyrian2000-Enhanced.x86_64"
WINDOW_NAME = "OpenTyrian2000 Enhanced"
# Player data of each game: <home>/userdata, forced by --user-dir= (otherwise the
# shared package would write into its own userdata/ and the games of a batch would get mixed up).
USER_DIR = "userdata"
# Two games of the same batch must not take the same Xvfb screen.
_DISPLAY_LOCK = threading.Lock()
_DISPLAYS_TAKEN = set()
# Live games of the driver: stopped if the driver itself is stopped (timeout,
# Ctrl+C). Otherwise the game, in its own session, survives and runs on a package
# that may be rebuilt underneath it.
_LIVE_GAMES = set()


def stop_games_on_signal():
    """SIGTERM, SIGINT, SIGHUP: stops the games and their Xvfbs, then exits (128 + signal)."""

    def handler(signum, _frame):
        for g in list(_LIVE_GAMES):
            g.kill()
        os._exit(128 + signum)

    for s in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(s, handler)


class JourneyFailure(Exception):
    """The game did not reach the wanted state: the journey stops there."""


class Game:
    """An instance of the game under its own Xvfb, driven by xdotool."""

    def __init__(self, package, out, size, home, extra_args=(), tag=""):
        self.package = Path(package)
        self.out = Path(out)
        self.width, self.height = size
        self.home = Path(home)
        self.extra_args = list(extra_args)
        self.tag = tag
        self.state_path = self.out / f"state-{tag}.json"
        self.log_path = self.out / f"godot-{tag}.log"
        self.xvfb = None
        self.proc = None
        self.display = None
        self.wid = None
        self.scale = min(self.width // 320, self.height // 200)
        self.ox = (self.width - 320 * self.scale) // 2
        self.oy = (self.height - 200 * self.scale) // 2
        self.mouse = (self.width // 2, self.height // 2)
        # Lightened journeys (--core): only the essential points.
        self.light = False
        self.free_window = False
        # Top-left corner of the window on the X screen (0, 0 except for a free window).
        self.win_pos = (0, 0)

    # --- Processes -------------------------------------------------------
    def start(self):
        with _DISPLAY_LOCK:
            first = 300 + os.getpid() % 400
            for n in range(first, first + 300):
                if n in _DISPLAYS_TAKEN:
                    continue
                if not os.path.exists(f"/tmp/.X11-unix/X{n}") and not os.path.exists(f"/tmp/.X{n}-lock"):
                    self.display = n
                    _DISPLAYS_TAKEN.add(n)
                    break
        _LIVE_GAMES.add(self)
        self.xvfb = subprocess.Popen(
            [
                "Xvfb",
                f":{self.display}",
                "-screen",
                "0",
                f"{self.width}x{self.height}x24",
                "-nolisten",
                "tcp",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        for _ in range(40):
            if os.path.exists(f"/tmp/.X11-unix/X{self.display}"):
                break
            time.sleep(0.25)
        self.env = dict(
            os.environ,
            DISPLAY=f":{self.display}",
            HOME=str(self.home),
            PULSE_SERVER="",
            GODOT_SILENCE_ROOT_WARNING="1",
        )
        self.env.pop("XDG_CONFIG_HOME", None)
        self.env.pop("XDG_DATA_HOME", None)
        self.launch()

    def launch(self):
        if self.state_path.exists():
            self.state_path.unlink()
        log = open(self.log_path, "a")
        # Free window (free_window): the game opens the window its profile remembered,
        # without "--windowed" or repositioning by the driver ("remembered-window" journey).
        forced = [] if self.free_window else ["--windowed"]
        args = (
            [f"./{EXE}", "--audio-driver", "Dummy", "--max-fps", "70", "--"]
            + forced
            + [f"--journey-state={self.state_path}", f"--user-dir={self.user_dir()}"]
            + self.extra_args
        )
        self.proc = subprocess.Popen(
            args,
            cwd=self.package,
            env=self.env,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        self.wid = None
        for _ in range(60):
            self.wid = self._find_window()
            if self.wid or self.proc.poll() is not None:
                break
            time.sleep(0.5)
        if not self.wid:
            raise JourneyFailure("game window not found")
        # The window is recreated at engine startup: take it back once the state is written.
        self.wait(lambda s: s.get("scene"), 20, "first scene")
        self.wid = self._find_window() or self.wid
        if self.free_window:
            time.sleep(2.0)
            # Window left where the game put it: clicks target the X screen, offset accordingly.
            g = subprocess.run(
                ["xdotool", "getwindowgeometry", "--shell", self.wid],
                env=self.env,
                capture_output=True,
                text=True,
            ).stdout
            vals = dict(line.split("=", 1) for line in g.split() if "=" in line)
            self.win_pos = (int(vals.get("X", 0)), int(vals.get("Y", 0)))
            return
        w, h = str(self.width), str(self.height)
        for _ in range(10):
            self.x("windowsize", self.wid, w, h)
            self.x("windowmove", self.wid, "0", "0")
            time.sleep(0.3)
            if self.state().get("window", {}).get("size") == [self.width, self.height]:
                break
        self.x("windowfocus", self.wid)
        time.sleep(0.3)

    def _find_window(self):
        r = subprocess.run(
            ["xdotool", "search", "--pid", str(self.proc.pid), "--name", WINDOW_NAME],
            env=self.env,
            capture_output=True,
            text=True,
        )
        ids = r.stdout.split()
        return ids[-1] if ids else None

    def alive(self):
        return self.proc is not None and self.proc.poll() is None

    def quit(self, timeout=10):
        """Waits for the game to end; returns the exit code (None if it is still alive)."""
        try:
            return self.proc.wait(timeout)
        except subprocess.TimeoutExpired:
            return None

    def kill(self):
        for p in (self.proc, self.xvfb):
            if p is not None and p.poll() is None:
                try:
                    os.killpg(p.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                try:
                    p.wait(5)
                except subprocess.TimeoutExpired:
                    os.killpg(p.pid, signal.SIGKILL)
        self.proc = None

    def close(self):
        self.kill()
        _LIVE_GAMES.discard(self)
        self.xvfb = None
        with _DISPLAY_LOCK:
            _DISPLAYS_TAKEN.discard(self.display)

    # --- Inputs ---------------------------------------------------------
    def x(self, *args):
        subprocess.run(["xdotool", *args], env=self.env, capture_output=True)

    def key(self, *names, pause=0.35):
        for k in names:
            self.x("key", "--delay", "60", k)
            time.sleep(pause)

    def hold(self, keys, seconds):
        for k in keys:
            self.x("keydown", k)
        time.sleep(seconds)
        for k in keys:
            self.x("keyup", k)

    def gx(self, x):
        w = self.state().get("window")
        if w:
            return int(self.win_pos[0] + w["origin"][0] + (x + 0.5) * w["pixel"][0])
        return self.ox + x * self.scale + self.scale // 2

    def gy(self, y):
        w = self.state().get("window")
        if w:
            return int(self.win_pos[1] + w["origin"][1] + (y + 0.5) * w["pixel"][1])
        return self.oy + y * self.scale + self.scale // 2

    def move(self, x, y):
        """Mouse to the point (x, y) of the 320x200 game, in steps of 4 window px."""
        tx, ty = self.gx(x), self.gy(y)
        cx, cy = self.mouse
        steps = max(abs(tx - cx), abs(ty - cy)) // 4 + 1
        cmd = []
        for i in range(1, steps + 1):
            cmd += [
                "mousemove",
                str(cx + (tx - cx) * i // steps),
                str(cy + (ty - cy) * i // steps),
                "sleep",
                "0.004",
            ]
        self.x(*cmd)
        self.mouse = (tx, ty)
        time.sleep(0.25)

    def click(self, x, y, button=1):
        self.move(x, y)
        self.x("click", str(button))
        time.sleep(0.4)

    # --- State ------------------------------------------------------------
    def state(self):
        try:
            return json.loads(self.state_path.read_text())
        except (OSError, ValueError):
            return {}

    def wait(self, pred, timeout=8.0, what="expected state"):
        """Waits until pred(state) is true; returns the state, raises JourneyFailure otherwise."""
        end = time.time() + timeout
        s = {}
        while time.time() < end:
            if not self.alive():
                raise JourneyFailure(f"the game stopped while waiting for: {what}")
            s = self.state()
            try:
                if s and pred(s):
                    return s
            except (KeyError, IndexError, TypeError):
                pass
            time.sleep(0.15)
        raise JourneyFailure(f"timeout ({timeout:.0f} s) while waiting for: {what}; scene {s.get('scene')}")

    def settle(self, seconds=0.6):
        """Lets at least two state readouts go by after an action."""
        time.sleep(seconds)
        return self.state()

    def screenshot(self, path):
        subprocess.run(
            ["import", "-display", f":{self.display}", "-window", "root", str(path)],
            env=self.env,
            capture_output=True,
        )

    def user_dir(self):
        return self.home / USER_DIR
