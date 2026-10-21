#!/usr/bin/env python3
"""Regression tests for the utils/ tool chain (stdlib unittest, no deps).

Every case here is a bug that an audit round actually found once: each
was a three-line assertion away from never coming back. Run with

    python3 -m unittest discover -s utils/tests -v

The tests read the repo's real data/ (tyrian.hdt, tyrian.shp,
sprite_manifest.json) and never write into it: every
fixture goes to a temporary directory.
"""

import contextlib
import io
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

UTILS = Path(__file__).resolve().parents[1]
ROOT = UTILS.parent
sys.path.insert(0, str(UTILS))

import _json5  # noqa: E402
import _shared  # noqa: E402
import _sprite_manifest as sm  # noqa: E402

PY = sys.executable


def run(script, *args, cwd=None):
    """Run a util script in a subprocess; return CompletedProcess."""
    return subprocess.run(
        [PY, str(UTILS / script), *map(str, args)],
        capture_output=True,
        text=True,
        cwd=cwd or str(ROOT),
    )


class TempDirMixin:
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="ot2k_utils_"))
        self.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)

    def mod(self, root, name="m", raw=None):
        """Write a mod folder with the given mod.json object (or raw bytes)."""
        d = self.tmp / name
        d.mkdir(parents=True, exist_ok=True)
        if raw is None:
            raw = json.dumps(root).encode("utf-8")
        (d / "mod.json").write_bytes(raw)
        return d


class Json5Tests(unittest.TestCase):
    def test_bom_is_tolerated_like_cjson(self):  # A08
        self.assertEqual(_json5.loads('﻿{"a": 1}'), {"a": 1})
        self.assertEqual(_json5.loads(b'\xef\xbb\xbf{"a": 1}'), {"a": 1})

    def test_bom_from_file_path_and_handle(self):
        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        p = d / "x.json"
        p.write_bytes(b'\xef\xbb\xbf{"k": [1, 2,], // c\n}')
        self.assertEqual(_json5.load(str(p)), {"k": [1, 2]})
        with open(p, encoding="utf-8") as f:
            self.assertEqual(_json5.load(f), {"k": [1, 2]})

    def test_comments_and_trailing_commas(self):
        self.assertEqual(_json5.loads('{/* x */"a": [1,], // y\n}'), {"a": [1]})


class SpriteManifestTests(unittest.TestCase):
    def test_weapon_itemgraphic_resolves_in_pickups_section(self):  # RSI-1 M3
        keys = sm.SECTION_MANIFEST_KEYS["weapons"]
        self.assertEqual(keys[0], "pickups")
        manifest = sm.load_manifest()
        self.assertTrue(manifest, "data/sprite_manifest.json unreadable")
        self.assertEqual(sm.resolve_index(manifest, 7, keys), "weapons/pulse_cannon")


class LocaleTablesTests(unittest.TestCase):
    def test_curly_apostrophe_maps_to_ascii_apostrophe(self):  # A04 / A05
        conv = _shared.utf8_to_cp437_map()
        self.assertEqual(conv["’"], 0x27)  # ’ -> '  (was 0x5C: '\'' read as '\')
        self.assertEqual(conv["‘"], 0x27)
        self.assertEqual(conv["–"], ord("-"))  # – (3-byte block, missing in the old copy)
        self.assertEqual(conv["—"], ord("-"))
        self.assertEqual(conv["…"], ord("."))
        self.assertEqual(conv["é"], 0x82)  # é
        self.assertEqual(conv["¿"], 0xA8)  # ¿ (0xC2 block)
        self.assertGreaterEqual(len(conv), 70)

    def test_font_tables_are_frozen_c_tables(self):
        fa, unacc = _shared.font_tables()
        self.assertEqual(len(fa), 256)
        self.assertEqual(unacc[0x82], "e")
        self.assertEqual(_shared.text_to_cp437("a’b°"), [97, 0x27, 98, None])

    def test_the_three_lints_share_one_table(self):
        import font_glyph_lint  # noqa: F401
        import locale_glyphs
        import locale_widths

        for mod in (font_glyph_lint, locale_glyphs, locale_widths):
            self.assertFalse(hasattr(mod, "CONV"), mod.__name__)
            self.assertIs(mod.utf8_to_cp437_map, _shared.utf8_to_cp437_map)
            self.assertIs(mod.font_tables, _shared.font_tables)

    def test_datacube_check_accepts_typographic_punctuation(self):  # A04 fixture
        import locale_cubes as lc

        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        (d / "locale_en_cubes.txt").write_text(
            "dc_1_1_title=T\ndc_1_1_header=H\ndc_1_1_body=`Hello\n`\n", encoding="utf-8"
        )
        (d / "locale_fr_cubes.txt").write_text(
            "dc_1_1_title=T\ndc_1_1_header=H\ndc_1_1_body=`L’audit – test…\n`\n", encoding="utf-8"
        )
        old = lc.LOC, lc.DATA
        lc.LOC, lc.DATA = str(d), str(d)  # no cubetxt*.dat -> key parity skipped
        try:
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                rc = lc.check()
        finally:
            lc.LOC, lc.DATA = old
        self.assertEqual(rc, 0, out.getvalue())
        self.assertNotIn("U+2019", out.getvalue())


class AuditLeftoversTests(unittest.TestCase):
    def test_large_font_keys_have_one_source(self):  # A06
        import font_glyph_lint
        import locale_widths

        lint_keys = font_glyph_lint.large_font_keys()
        self.assertIn("superShip_0", lint_keys)  # array backed by a bulk
        self.assertIn("endless_summary_title", lint_keys)  # L("cle") direct
        self.assertTrue(lint_keys <= locale_widths.large_font_keys())


class HelpDoesNotRunTheCheck(unittest.TestCase):  # A19
    SCRIPTS = (
        "locale_check.py",
        "locale_glyphs.py",
        "locale_widths.py",
        "font_glyph_lint.py",
        "shop_coverage.py",
    )

    def test_help_prints_usage_only(self):
        for s in self.SCRIPTS:
            r = run(s, "--help")
            self.assertEqual(r.returncode, 0, s + ": " + r.stderr)
            self.assertTrue(r.stdout.startswith("usage:"), s + ": " + r.stdout[:80])
            self.assertNotIn(" OK", r.stdout, s)

    def test_locale_check_strict_rejects_empty_values(self):
        import locale_check

        d = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, d, ignore_errors=True)
        (d / "locale_x.txt").write_text("a=1\nb=\nnot a line\n", encoding="utf-8")
        del locale_check.STRICT_PROBLEMS[:]
        with contextlib.redirect_stderr(io.StringIO()):
            keys = locale_check.load_keys(d / "locale_x.txt", strict=True)
        self.assertEqual(set(keys), {"a", "b"})
        self.assertEqual(len(locale_check.STRICT_PROBLEMS), 2)


if __name__ == "__main__":
    unittest.main()
