#!/usr/bin/env python3
"""Delivery unit tests; the cargo/Godot doubles are NOT a real build."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent


class VisualTests(unittest.TestCase):
    def test_empty_otfr_sequence(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = subprocess.run(
                [sys.executable, str(SCRIPTS / "visual_diff.py"), "seqdiff", tmp, tmp],
                capture_output=True,
            )
            self.assertNotEqual(result.returncode, 0)

    def test_hash_reference(self):
        # Versioned reference (seqhash): identical -> 0, one changed pixel -> 1.
        import struct

        def otfr(path, color):
            pal = bytes([0, 0, 0, color, 0, 0] + [0] * 762)
            path.write_bytes(b"OTFR" + struct.pack("<IHH", 0, 2, 2) + bytes([0, 1, 1, 1]) + pal + bytes(16))

        with tempfile.TemporaryDirectory() as tmp:
            ref, cur = Path(tmp, "ref"), Path(tmp, "cur")
            ref.mkdir()
            cur.mkdir()
            otfr(ref / "000000.otfr", 200)
            otfr(cur / "000000.otfr", 200)
            tool = [sys.executable, str(SCRIPTS / "visual_diff.py")]
            sha = Path(tmp, "ref.sha1")
            sha.write_text(
                subprocess.run(tool + ["seqhash", str(ref)], capture_output=True, text=True, check=True).stdout
            )
            self.assertEqual(subprocess.run(tool + ["seqdiff", str(sha), str(cur)]).returncode, 0)
            otfr(cur / "000000.otfr", 201)
            self.assertEqual(subprocess.run(tool + ["seqdiff", str(sha), str(cur)]).returncode, 1)
            otfr(cur / "000000.otfr", 200)
            otfr(cur / "000001.otfr", 200)  # extra image on the Godot side
            self.assertEqual(subprocess.run(tool + ["seqdiff", str(sha), str(cur)]).returncode, 1)


@unittest.skipUnless(os.name == "nt", "PowerShell unit tests run on Windows")
class WindowsTests(unittest.TestCase):
    def test_native_failures(self):
        with tempfile.TemporaryDirectory(prefix="delivery-unit-win-") as tmp:
            root = Path(tmp)
            scripts = root / "rewrite/scripts"
            scripts.mkdir(parents=True)
            shutil.copy2(SCRIPTS / "build-windows.ps1", scripts)
            target = root / "target"
            dll = target / "x86_64-pc-windows-msvc/release/tyrian_godot.dll"
            dll.parent.mkdir(parents=True)
            dll.write_text("unit fixture DLL, never loaded")
            (root / "cargo.cmd").write_text("@exit /b %CARGO_CODE%\n")
            (root / "godot.cmd").write_text(
                '@echo %* | findstr /c:"--import" >nul\n@if errorlevel 1 exit /b 0\n@echo %GODOT_MESSAGE%\n@exit /b %GODOT_CODE%\n'
            )
            for cargo, godot, message in [
                ("7", "0", "fixture"),
                ("0", "9", "fixture"),
                ("0", "0", "SCRIPT ERROR: fixture"),
                ("0", "0", "fixture"),
            ]:
                with self.subTest(cargo=cargo, godot=godot, message=message):
                    env = dict(
                        os.environ,
                        PATH=str(root) + os.pathsep + os.environ["PATH"],
                        CARGO_TARGET_DIR=str(target),
                        CARGO_CODE=cargo,
                        GODOT_CODE=godot,
                        GODOT_MESSAGE=message,
                    )
                    result = subprocess.run(
                        [
                            "powershell.exe",
                            "-NoProfile",
                            "-ExecutionPolicy",
                            "Bypass",
                            "-File",
                            str(scripts / "build-windows.ps1"),
                            "-Godot",
                            str(root / "godot.cmd"),
                            "-SkipImport",
                        ],
                        env=env,
                        capture_output=True,
                        text=True,
                        errors="replace",
                    )
                    if cargo == godot == "0" and message == "fixture":
                        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    else:
                        self.assertNotEqual(result.returncode, 0, result.stdout)
                        self.assertNotIn("OK: run", result.stdout)


@unittest.skipUnless(os.name == "posix", "shell unit tests run on Linux/WSL")
class ShellTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="delivery-unit-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.scripts = self.root / "rewrite/scripts"
        self.scripts.mkdir(parents=True)
        for name in ("build-linux.sh", "package.sh", "godot-checked.sh"):
            if (SCRIPTS / name).exists():
                shutil.copy2(SCRIPTS / name, self.scripts / name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.env = dict(
            os.environ,
            PATH=str(self.bin) + ":" + os.environ["PATH"],
            GODOT=str(self.bin / "godot"),
            OUT=str(self.root / "out"),
            XDG_DATA_HOME=str(self.root / "xdg"),
            CARGO_TARGET_DIR=str(self.root / "target"),
        )
        self.put("target/release/libtyrian_godot.so", "unit fixture")
        self.put("COPYING", "code license fixture")
        self.put("rewrite/licenses/rustysynth-LICENSE.txt", "MIT License, Nobuaki Tanaka (fixture)")
        self.put("rewrite/licenses/TimGM6mb-LICENSE.txt", "TimGM6mb, Tim Brechbill, GPL v2 (fixture)")
        self.put("rewrite/godot/project.godot", 'config/version="v1.0-rc1"\n')
        self.put(
            "rewrite/godot/export_presets.cfg",
            'application/file_version="1.0.0.1"\napplication/product_version="1.0.0.1"\n',
        )
        self.put("rewrite/godot/icon/tyrian2000-32.png", "icon fixture")
        self.put("rewrite/packaging/README.txt", "player README fixture")
        self.put("rewrite/packaging/linux/opentyrian2000-enhanced.desktop", "desktop entry fixture")
        self.put("rewrite/packaging/linux/install-desktop-entry.sh", "install fixture")
        # Example mods copied next to the executable (package.sh copy_data).
        self.put("rewrite/mods/README.md", "mods README fixture")
        self.put("rewrite/mods/mod.schema.json", "{}\n")
        for name in (
            "linux_release.x86_64",
            "windows_release_x86_64.exe",
            "windows_release_x86_64_console.exe",
        ):
            self.put("xdg/godot/export_templates/4.7.2.stable/" + name, "template fixture")
        self.executable("cargo", 'exit "${CARGO_CODE:-0}"')
        # The fixture libraries are not binaries: strip is a double with no effect.
        self.executable("strip", "exit 0")
        self.executable("x86_64-w64-mingw32-strip", "exit 0")
        # Double of hd_assets.sh: the real HD archive (94 MiB) is not
        # downloaded by the unit tests.
        self.put("rewrite/scripts/hd_assets.sh", 'exit "${HD_CODE:-0}"\n')
        for name in (
            "sprites/a.png",
            "maps/0123456789abcdef/000.webp",
            "pics/a.png",
            "manifest.json",
            "sources.sha256",
        ):
            self.put("rewrite/godot/assets/upscaled/" + name, "fixture HD")
        self.executable(
            "godot",
            """
if [[ "$*" == *--export-release* ]]; then
    dest="${!#}"
    touch "$dest" "$(dirname "$dest")/libtyrian_godot.so"
fi
if [[ "$*" == *--import* || "$*" == *--export-release* ]]; then
    printf '%s\\n' "${GODOT_MESSAGE:-unit fixture Godot}"
    exit "${GODOT_CODE:-0}"
fi
exit 0
""",
        )

    def put(self, path, content):
        dest = self.root / path
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content)

    def executable(self, name, body):
        dest = self.bin / name
        dest.write_text("#!/usr/bin/env bash\n" + body + "\n")
        dest.chmod(0o755)

    def run_script(self, name, *args, **env):
        return subprocess.run(
            ["bash", str(self.scripts / name), *args],
            env=dict(self.env, **env),
            capture_output=True,
            text=True,
        )

    def test_build_failure_propagation(self):
        for env in (
            {"CARGO_CODE": "7"},
            {"GODOT_CODE": "9"},
            {"GODOT_MESSAGE": "SCRIPT ERROR: deliberate fixture"},
        ):
            with self.subTest(env=env):
                result = self.run_script("build-linux.sh", "--skip-import", **env)
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertNotIn("OK: run", result.stdout)
        self.assertEqual(self.run_script("build-linux.sh", "--skip-import").returncode, 0)

    def test_missing_ui_capture_is_failure(self):
        for name in ("ui_regression.sh", "visual_diff.py"):
            shutil.copy2(SCRIPTS / name, self.scripts / name)
        capture = self.scripts / "visual_capture_godot.sh"
        capture.write_text("#!/usr/bin/env bash\nexit 7\n")
        capture.chmod(0o755)
        result = self.run_script(
            "ui_regression.sh",
            "godot",
            str(self.root / "captures"),
            str(self.root / "reference"),
            "",
            "title",
        )
        self.assertNotEqual(result.returncode, 0)

    def test_package_invalid_only(self):
        for value in ("invalid", "", "Linux"):
            result = self.run_script("package.sh", "--skip-build", "--only=" + value)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((self.root / "out").exists())

    def test_package_macos_needs_a_macos_host(self):
        # Experimental macOS package: lipo, codesign and ditto only exist on macOS;
        # refused before any build or download, never a half-made bundle.
        result = self.run_script("package.sh", "--skip-build", "--only=macos")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("macOS host", result.stderr)
        self.assertFalse((self.root / "out").exists())

    def test_export_script_error(self):
        self.put("data/tyrian.hdt", "unit fixture")
        result = self.run_script(
            "package.sh",
            "--skip-build",
            "--only=linux",
            GODOT_MESSAGE="SCRIPT ERROR: deliberate fixture",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(list((self.root / "out").rglob("*.tar.gz")))

    def test_package_hd_failure(self):
        self.put("data/tyrian.hdt", "unit fixture")
        result = self.run_script("package.sh", "--skip-build", "--only=linux", HD_CODE="3")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(list((self.root / "out").rglob("*.tar.gz")))

    def test_package_data_and_notices(self):
        for name in (
            "tyrian.hdt",
            "tyrian1.lvl",
            "levels1.dat",
            "tyrian.shp",
            "tyrianc.shp",
            "tyrian.pic",
            "palette.dat",
            "newsh1.shp",
            "shapesa.dat",
            "locales/locale_fr.txt",
            "sprite_manifest.json",
            "demo.1",
            "cubetxt1.dat",
            "tyrend.anm",
            "tyrian.cdt",
            "estsc.shp",
            "tshp2.pcx",
            "midi/29-title.mid",
            "midi/notes.txt",
            "soundfont.sf2",
            "hd/remastered/a.shp",
            "unknown.secret",
        ):
            self.put("data/" + name, "data fixture")
        result = self.run_script("package.sh", "--skip-build", "--only=linux")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        dest = self.root / "out/linux/OpenTyrian2000-Enhanced"
        self.assertTrue((dest / "data/tyrian.hdt").exists())
        # Title demo and data cube texts: read at runtime.
        for name in (
            "demo.1",
            "cubetxt1.dat",
            "tyrend.anm",
            "tyrian.cdt",
            "estsc.shp",
            "tshp2.pcx",
        ):
            self.assertTrue((dest / "data" / name).exists(), name)
        # MIDI music (ADR 0130): "NN-*.mid" tracks and SoundFont only.
        self.assertTrue((dest / "data/midi/29-title.mid").exists())
        self.assertTrue((dest / "data/soundfont.sf2").exists())
        for name in ("README.txt", "opentyrian2000-enhanced.desktop", "icons/tyrian2000-32.png"):
            self.assertTrue((dest / name).exists(), name)
        self.assertIn("Nobuaki Tanaka", (dest / "licenses/rustysynth-LICENSE.txt").read_text())
        self.assertIn("Tim Brechbill", (dest / "licenses/TimGM6mb-LICENSE.txt").read_text())
        for name in ("midi/notes.txt", "hd", "unknown.secret"):
            self.assertFalse((dest / "data" / name).exists(), name)
        self.assertTrue((dest / "upscaled/maps/0123456789abcdef/000.webp").exists())
        self.assertTrue((dest / "upscaled/pics/a.png").exists())
        notice = (dest / "DATA-NOTICES.txt").read_text()
        for term in (
            "freeware",
            "GPL",
            "redistribution",
            "soundfont.sf2",
            "midi",
            "NOT INCLUDED",
            "rustysynth",
        ):
            self.assertIn(term, notice)

    def test_package_ships_the_mod_packs_but_not_the_retired_ones(self):
        # package.sh copies each rewrite/mods/<folder>/ that holds a mod.json; _retired/
        # (examples kept for reference, rewrite/mods/README.md) has none at its root.
        self.put("data/tyrian.hdt", "unit fixture")
        for mod in ("ship_pack", "weapon_pack", "_retired/old_example"):
            self.put(f"rewrite/mods/{mod}/mod.json", "{}")
        self.put("rewrite/mods/ship_pack/sprites/ships/a.png", "png fixture")
        self.put("rewrite/mods/notes/notes.txt", "not a mod")
        result = self.run_script("package.sh", "--skip-build", "--only=linux")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        mods = self.root / "out/linux/OpenTyrian2000-Enhanced/mods"
        for name in (
            "README.md",
            "mod.schema.json",
            "ship_pack/mod.json",
            "ship_pack/sprites/ships/a.png",
            "weapon_pack/mod.json",
        ):
            self.assertTrue((mods / name).exists(), name)
        for name in ("_retired", "old_example", "notes"):
            self.assertFalse((mods / name).exists(), name)


if __name__ == "__main__":
    unittest.main(verbosity=2)
