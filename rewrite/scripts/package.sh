#!/usr/bin/env bash
# Ready-to-play packages (blocks 13 and 15): Godot export + GDExtension + data/.
#
#   build/windows/OpenTyrian2000-Enhanced/  OpenTyrian2000-Enhanced.exe (Tyrian icon
#                                    and metadata set by rcedit), .pck, README.txt,
#                                    COPYING, tyrian_godot.dll (mingw cross-compiled), data/
#   build/linux/OpenTyrian2000-Enhanced/    OpenTyrian2000-Enhanced.x86_64, .pck, README.txt,
#                                    COPYING, libtyrian_godot.so, data/, icons/, desktop entry
#   + a .zip (Windows) and .tar.gz (Linux / Steam Deck) archive of each folder,
#     named after the version of project.godot (application/config/version):
#     OpenTyrian2000-Enhanced-<version>-windows-x86_64.zip and
#     OpenTyrian2000-Enhanced-<version>-linux-x86_64.tar.gz, e.g. <version> = v1.0-rc1.
#     The folder inside each archive stays OpenTyrian2000-Enhanced/.
#   build/macos/OpenTyrian2000 Enhanced.app (EXPERIMENTAL, --only=macos, macOS host
#                                    only): universal app (x86_64 + arm64), GDExtension in
#                                    Contents/Frameworks/, data/, mods/, upscaled/, notices in
#                                    Contents/Resources/ (game_paths.gd), ad-hoc signature
#                                    (codesign -s -, no Apple identity, not notarized),
#                                    zipped by ditto: OpenTyrian2000-Enhanced-<version>-macos-universal.zip.
#                                    Not part of the default run (Windows + Linux).
#
# Steps: scripts/build-linux.sh --windows (libraries, tyrian-import,
# level scenes) unless --skip-build; installation of the 4.7.2 export
# templates if missing; `godot --headless --export-release` of the
# "Windows Desktop" and "Linux" presets (godot/export_presets.cfg); copy of data/.
# HD (AI) mode images (ADR 0100): included in upscaled/ (except --lite),
# downloaded and checked by fingerprint (scripts/hd_assets.sh fetch), never
# rendered on the local GPU. Options: --skip-build, --only=windows|linux|macos.
#
# Lite variant:
#   --lite      packages WITHOUT upscaled/ (HD (AI) not offered until the pack
#               is installed): build/{windows,linux}-lite/OpenTyrian2000-Enhanced/,
#               archives OpenTyrian2000-Enhanced-<version>-{windows,linux}-x86_64-lite.{zip,tar.gz}.
#               The HD archive is then neither downloaded nor required.
#   --hd-pack   in addition: build/hd-pack/OpenTyrian2000-Enhanced-<version>-hd-pack.zip, the
#               verified upscaled/ folder (hd_assets.sh fetch) to unzip into the game folder.
#   Without these options: full packages, as before (HD included).
#
# Windows executable icon: Godot calls rcedit.exe, which is a Windows
# program. From WSL it is launched natively by rcedit-wsl.sh (no window);
# elsewhere on Linux it goes through wine, which must exist. rcedit is downloaded
# once (fingerprint checked) into ~/.cache/rcedit. Without rcedit the Windows
# export fails: never an executable without an icon shipped silently.
#
# Variables: GODOT (default godot), CARGO_TARGET_DIR, OUT (default <repo>/build),
# GODOT_TEMPLATES_TPZ (.tpz archive already downloaded, otherwise download),
# RCEDIT_EXE (path of an rcedit-x64.exe already present).
# Usage: rewrite/scripts/package.sh [--skip-build] [--only=linux] [--lite] [--hd-pack]
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
rewrite="$(dirname "$here")"
repo="$(dirname "$rewrite")"
godot_dir="$rewrite/godot"
GODOT="${GODOT:-godot}"
# shellcheck source-path=SCRIPTDIR
source "$here/godot-checked.sh"
OUT="${OUT:-$repo/build}"
name=OpenTyrian2000-Enhanced
version=4.7.2.stable
# Godot looks for its export templates in its data folder: XDG on Linux,
# ~/Library/Application Support/Godot on macOS.
if [ "$(uname -s)" = Darwin ]; then
    tpl_dir="$HOME/Library/Application Support/Godot/export_templates/$version"
else
    tpl_dir="${XDG_DATA_HOME:-$HOME/.local/share}/godot/export_templates/$version"
fi
tpz_url="https://github.com/godotengine/godot/releases/download/4.7.2-stable/Godot_v4.7.2-stable_export_templates.tpz"
# Official fingerprint (SHA512-SUMS.txt of the 4.7.2-stable release).
tpz_sha512="ca4d71c4d7b81dfc15d1a98baa07534aa95b03fdda78a0075b06672e1648d2e5f40980c9adc28d23e1b92e732ee7bf3461997aa804af74ec2fcd7a93ccb84079"
rcedit_url="https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe"
rcedit_sha256="3e7801db1a5edbec91b49a24a094aad776cb4515488ea5a4ca2289c400eade2a"
export GODOT_SILENCE_ROOT_WARNING=1

skip_build=0
only=""
lite=0
hd_pack=0
for arg in "$@"; do
    case "$arg" in
        --skip-build) skip_build=1 ;;
        --only=windows|--only=linux|--only=macos) only="${arg#--only=}" ;;
        --lite) lite=1 ;;
        --hd-pack) hd_pack=1 ;;
        *) echo "unknown argument: $arg" >&2; exit 2 ;;
    esac
done

# Single version: application/config/version of the project. The version fields of
# the Windows executable (preset) must follow it, as four numbers: major.minor.patch.rc
# (v1.0-rc1 -> 1.0.0.1; a final v1.0 -> 1.0.0.0). The archive names carry the version
# with its "v" (release tag): OpenTyrian2000-Enhanced-v1.0-rc1-...
project_version="$(sed -n 's/^config\/version="v\{0,1\}\([^"]*\)"/\1/p' "$godot_dir/project.godot")"
[ -n "$project_version" ] || { echo "config/version not found in project.godot" >&2; exit 1; }
release="$name-v$project_version"
numeric_version="$(python3 -c 'import re,sys; m = re.fullmatch(r"v?(\d+)\.(\d+)(?:\.(\d+))?(?:-rc(\d+))?", sys.argv[1]); assert m, sys.argv[1]; print(".".join(x or "0" for x in m.groups()))' "$project_version")"
for key in file_version product_version; do
    got="$(sed -n "s/^application\/$key=\"\(.*\)\"/\1/p" "$godot_dir/export_presets.cfg" | head -n1)"
    [ "$got" = "$numeric_version" ] || {
        echo "export_presets.cfg: application/$key=$got, expected $numeric_version (version $project_version)" >&2
        exit 1
    }
done

# macOS (experimental): only on a macOS host (Apple linker, lipo, codesign,
# ditto). Same version: CFBundleVersion = the four numbers above,
# CFBundleShortVersionString = the first three.
if [ "$only" = macos ]; then
    [ "$(uname -s)" = Darwin ] || { echo "--only=macos needs a macOS host (lipo, codesign, ditto)" >&2; exit 1; }
    for pair in "version=$numeric_version" "short_version=${numeric_version%.*}"; do
        key="${pair%%=*}"
        got="$(sed -n "s/^application\/$key=\"\(.*\)\"/\1/p" "$godot_dir/export_presets.cfg" | head -n1)"
        [ "$got" = "${pair#*=}" ] || {
            echo "export_presets.cfg (macOS): application/$key=$got, expected ${pair#*=} (version $project_version)" >&2
            exit 1
        }
    done
fi

# Explicit "bash": the scripts are tracked as 100644 (core.filemode
# false on Windows/WSL workstations), a direct call fails in CI (code 126).
if [ "$skip_build" = 0 ]; then
    if [ "$only" = linux ]; then
        bash "$here/build-linux.sh"
    elif [ "$only" = macos ]; then
        bash "$here/build-macos.sh"
    else
        bash "$here/build-linux.sh" --windows
    fi
fi

# Export templates: only those of the target platforms are extracted
# (the official archive is 1.3 GB, 2 GB uncompressed).
if [ "$only" = macos ]; then
    tpl_needed=(macos.zip)
    tpl_patterns=(templates/macos.zip)
else
    tpl_needed=(linux_release.x86_64 windows_release_x86_64.exe windows_release_x86_64_console.exe)
    tpl_patterns=('templates/linux_*.x86_64' 'templates/windows_*_x86_64*')
fi
need_tpl() {
    local f
    for f in "${tpl_needed[@]}"; do
        [ -f "$tpl_dir/$f" ] || return 1
    done
}
if ! need_tpl; then
    tpz="${GODOT_TEMPLATES_TPZ:-}"
    if [ -z "$tpz" ]; then
        tpz="$(mktemp -d)/templates.tpz"
        echo "== downloading export templates $version"
        curl -sSLf -o "$tpz" "$tpz_url"
    fi
    # Official SHA-512 (python3: sha512sum is missing from macOS).
    got_sha="$(python3 -c 'import hashlib,sys; h = hashlib.sha512(); f = open(sys.argv[1], "rb"); [h.update(b) for b in iter(lambda: f.read(1 << 20), b"")]; print(h.hexdigest())' "$tpz")"
    [ "$got_sha" = "$tpz_sha512" ] || { echo "unexpected SHA-512 of the export templates: $tpz" >&2; exit 1; }
    tmp="$(mktemp -d)"
    unzip -oq "$tpz" "${tpl_patterns[@]}" templates/version.txt -d "$tmp"
    mkdir -p "$tpl_dir"
    cp "$tmp"/templates/* "$tpl_dir/"
    rm -rf "$tmp"
fi

# rcedit: Windows executable that sets the icon and the version fields.
# Returns on stdout an isolated Godot configuration folder (XDG_CONFIG_HOME)
# that points to rcedit and its launcher, without touching the user's
# configuration.
prepare_rcedit_config() {
    local rcedit="${RCEDIT_EXE:-$HOME/.cache/rcedit/rcedit-x64.exe}" runner cfg
    if [ ! -f "$rcedit" ]; then
        mkdir -p "$(dirname "$rcedit")"
        echo "== downloading rcedit" >&2
        curl -sSLf -o "$rcedit.part" "$rcedit_url"
        [ "$(sha256sum "$rcedit.part" | cut -d' ' -f1)" = "$rcedit_sha256" ] \
            || { echo "unexpected rcedit fingerprint" >&2; rm -f "$rcedit.part"; return 1; }
        mv "$rcedit.part" "$rcedit"
    fi
    chmod +x "$rcedit"
    if grep -qi microsoft /proc/version 2>/dev/null; then
        runner="$here/rcedit-wsl.sh"
    elif command -v wine >/dev/null 2>&1; then
        runner="$(command -v wine)"
    else
        echo "rcedit needs WSL or wine for the Windows export" >&2
        return 1
    fi
    cfg="$(mktemp -d)"
    mkdir -p "$cfg/godot"
    printf '%s\n' '[gd_resource type="EditorSettings" format=3]' '' '[resource]' \
        "export/windows/rcedit = \"$rcedit\"" \
        "export/windows/wine = \"$runner\"" >"$cfg/godot/editor_settings-4.7.tres"
    printf '%s' "$cfg"
}

# data/: the simulation reads tyrianN.lvl, tyrian.hdt... (tyrian-sim/src/setup.rs)
# in the data/ folder placed next to the executable (game.gd:_default_data_dir).
# Allowed list: setup/campaign, render/assets, godot/playfield, tyrian_text
# and the MIDI music (music.gd: midi/NN-*.mid and soundfont.sf2, ADR 0130).
# No recursive copy: third-party mods and unknown files excluded.
# This technical selection grants no redistribution right (docs/asset-rights.md).
copy_data() {
    local dest="$1" src
    mkdir -p "$dest/data/locales"
    for src in "$repo/data"/tyrian[1-5].lvl "$repo/data"/levels[1-5].dat \
        "$repo/data"/tyrian.hdt "$repo/data"/tyrian.shp "$repo/data"/tyrianc.shp \
        "$repo/data"/tyrian.pic "$repo/data"/palette.dat "$repo/data"/newsh?.shp \
        "$repo/data"/shapes?.dat "$repo/data"/sprite_manifest.json         "$repo/data"/demo.[1-5] "$repo/data"/cubetxt[1-5].dat \
        "$repo/data"/tyrend.anm "$repo/data"/tyrian.cdt "$repo/data"/estsc.shp "$repo/data"/tshp2.pcx; do
        [ -f "$src" ] || continue
        [ ! -L "$src" ] || { echo "data link refused: $src" >&2; return 1; }
        cp "$src" "$dest/data/"
    done
    if [ -f "$repo/data/soundfont.sf2" ]; then
        [ ! -L "$repo/data/soundfont.sf2" ] || { echo "SoundFont link refused" >&2; return 1; }
        cp "$repo/data/soundfont.sf2" "$dest/data/"
    fi
    [ ! -L "$repo/data/midi" ] || { echo "midi link refused" >&2; return 1; }
    for src in "$repo/data"/midi/[0-9][0-9]-*.mid; do
        [ -f "$src" ] || continue
        [ ! -L "$src" ] || { echo "MIDI link refused: $src" >&2; return 1; }
        mkdir -p "$dest/data/midi"
        cp "$src" "$dest/data/midi/"
    done
    [ ! -L "$repo/data/locales" ] || { echo "locales link refused" >&2; return 1; }
    for src in "$repo/data"/locales/locale_*.txt; do
        [ -f "$src" ] || continue
        [ ! -L "$src" ] || { echo "locale link refused: $src" >&2; return 1; }
        cp "$src" "$dest/data/locales/"
    done
    cp "$repo/COPYING" "$dest/"
    # Example mods (like the C's data/mods): folders with mod.json only, placed
    # next to the executable; mods.gd reads them along with the user's mods folder.
    local mod
    mkdir -p "$dest/mods"
    cp "$rewrite/mods/README.md" "$rewrite/mods/mod.schema.json" "$dest/mods/"
    for mod in "$rewrite/mods"/*/; do
        [ -f "$mod/mod.json" ] || continue
        [ ! -L "${mod%/}" ] || { echo "mod link refused: $mod" >&2; return 1; }
        cp -r "${mod%/}" "$dest/mods/"
    done
    cp "$rewrite/packaging/README.txt" "$dest/README.txt"
    # MIT license of rustysynth, linked into the GDExtension: its text must travel with
    # the binary (midly, Unlicense, does not require it). GPL v2 of the TimGM6mb
    # SoundFont copied above (data/soundfont.sf2), with its copyright notice.
    mkdir -p "$dest/licenses"
    cp "$rewrite/licenses/rustysynth-LICENSE.txt" "$rewrite/licenses/TimGM6mb-LICENSE.txt" "$dest/licenses/"
    printf '%s\n' \
        'Code: see COPYING (GPL). The Tyrian data is separate from the code.' \
        'Tyrian 2000 data: freeware since 2004, not covered by the GPL of the code.' \
        'No written licence for the data is held in this repository; this package is not a redistribution clearance.' \
        'Before any redistribution, check the permissions and credits with the rights holders.' \
        'Imported images, sounds and music, and the optional HD output, derive from this data.' \
        'data/midi/*.mid (MIDI music, Setup > Sound): third-party transcriptions (karlsitretro.com) of the music by' \
        'Alexander Brandon and Andreas Molnar; third-party licence NOT INCLUDED, rights not verified.' \
        'data/soundfont.sf2: TimGM6mb SoundFont (Tim Brechbill, David Bolton), GPL v2: licence text and' \
        'copyright notice in licenses/TimGM6mb-LICENSE.txt.' \
        'MIDI synthesiser: rustysynth (Nobuaki Tanaka, MIT licence, text in licenses/) and midly (Unlicense), linked into the game.' \
        'Left out of data/: third-party mods and files not needed at runtime.' \
        'upscaled/ (HD (AI) mode): 4x upscale by the 4x-PixelPerfectV4 model (mutin-choler, WTFPL licence,' \
        'https://openmodeldb.info/models/4x-PixelPerfectV4), ESRGAN architecture, run with spandrel (MIT) and PyTorch (BSD).' \
        'Sharp pixel-art filter: xBRZ freescale shader (libretro/glsl-shaders; Hyllian, MIT; Zenju, GPL v3).' \
        'Reference: docs/asset-rights.md of the source repository.' >"$dest/DATA-NOTICES.txt"
    # Lite package: no upscaled/ (separate HD pack, copy_upscaled).
    [ "$lite" = 1 ] || copy_upscaled "$dest"
}

# upscaled/ of the HD (AI) mode: sprites, scenery (maps), interface backgrounds (pics)
# and their fingerprints. Same content in the full package and the HD pack.
copy_upscaled() {
    local dest="$1"
    mkdir -p "$dest/upscaled"
    # pics/: upscaled interface backgrounds (interface HD mode).
    cp -r "$godot_dir/assets/upscaled/sprites" "$godot_dir/assets/upscaled/maps" "$godot_dir/assets/upscaled/pics" \
        "$godot_dir/assets/upscaled/manifest.json" "$godot_dir/assets/upscaled/sources.sha256" "$dest/upscaled/"
}

# Strips the debug symbols from the Rust libraries (the Godot executables
# of the export templates are already stripped).
strip_library() {
    local tool="$1" file="$2"
    if command -v "$tool" >/dev/null 2>&1; then
        "$tool" --strip-unneeded "$file"
    else
        echo "$tool missing: $file keeps its symbols" >&2
    fi
}

# A game launched from the output folder reads its .pck at offsets fixed at
# startup: replacing the package underneath it makes it crash at the next load
# (unreadable level.tscn then SIGSEGV in the GDScript interpreter). Refuse.
refuse_if_running() {
    local dest="$1" pid exe
    for pid in /proc/[0-9]*; do
        exe="$(readlink "$pid/exe" 2>/dev/null || true)"
        case "$exe" in
            "$dest"/*) echo "game running from $dest (PID ${pid#/proc/}): stop it before rebuilding the package" >&2; exit 1 ;;
        esac
    done
}

# Player data (godot/scripts/user_data.gd: userdata/ next to the executable,
# or hand-placed files at the root): never in a package to be shared.
PLAYER_DATA=(userdata opentyrian.dat setup.cfg display.cfg controls.cfg destruct.cfg mods.cfg
    highscores.json endless_leaderboard.json endless_run.json saves replays
    c_profile_imported c_achievements_imported)
refuse_player_data() {
    local dest="$1" found=() f
    for f in "${PLAYER_DATA[@]}"; do
        [ -e "$dest/$f" ] && found+=("$f")
    done
    while IFS= read -r f; do found+=("${f#"$dest"/}"); done \
        < <(find "$dest" -maxdepth 1 -name 'screenshot_*.bmp')
    if [ ${#found[@]} -gt 0 ]; then
        echo "player data in the package $dest: ${found[*]}" >&2
        exit 1
    fi
}

export_preset() {
    local preset="$1" dest="$2" file="$3"
    refuse_if_running "$(realpath -m "$dest")"
    rm -rf "$dest"
    mkdir -p "$dest"
    echo "== export \"$preset\" -> $dest/$file"
    # --export-release writes the executable, the .pck and copies the GDExtension
    # declared in tyrian.gdextension for the target platform.
    run_godot_checked --headless --path "$godot_dir" --export-release "$preset" "$dest/$file"
    [ -f "$dest/$file" ] || { echo "executable missing: $dest/$file" >&2; exit 1; }
    copy_data "$dest"
}

# HD PNGs of the versioned archive (SHA-256 fingerprint in hd_assets.sh):
# useless to the lite package, except to build the HD pack.
if [ "$lite" = 0 ] || [ "$hd_pack" = 1 ]; then
    bash "$here/hd_assets.sh" fetch
fi

# Lite package: separate folders and archives (-lite), never mixed up with
# the full package that player-smoke.sh and player-journeys.sh read by default.
suffix=""
[ "$lite" = 0 ] || suffix="-lite"
win_out="$OUT/windows$suffix"
linux_out="$OUT/linux$suffix"

mkdir -p "$OUT"
if [ -z "$only" ] || [ "$only" = windows ]; then
    [ -f "$godot_dir/bin/tyrian_godot.dll" ] || { echo "tyrian_godot.dll missing (build-linux.sh --windows)" >&2; exit 1; }
    rcedit_cfg="$(prepare_rcedit_config)"
    XDG_CONFIG_HOME="$rcedit_cfg" export_preset "Windows Desktop" "$win_out/$name" "$name.exe"
    rm -rf "$rcedit_cfg"
    [ -f "$win_out/$name/tyrian_godot.dll" ] || { echo "DLL not copied by the export" >&2; exit 1; }
    strip_library x86_64-w64-mingw32-strip "$win_out/$name/tyrian_godot.dll"
    # The icon must be in the executable: otherwise the export ignored rcedit.
    if command -v wrestool >/dev/null 2>&1; then
        wrestool -l --type=14 "$win_out/$name/$name.exe" | grep -q group_icon \
            || { echo "icon missing from $name.exe (rcedit had no effect)" >&2; exit 1; }
    fi
    refuse_player_data "$win_out/$name"
    (cd "$win_out" && rm -f "$release-windows-x86_64$suffix.zip" && zip -qr "$release-windows-x86_64$suffix.zip" "$name")
fi
if [ -z "$only" ] || [ "$only" = linux ]; then
    export_preset "Linux" "$linux_out/$name" "$name.x86_64"
    [ -f "$linux_out/$name/libtyrian_godot.so" ] || { echo "libtyrian_godot.so not copied by the export" >&2; exit 1; }
    strip_library strip "$linux_out/$name/libtyrian_godot.so"
    chmod +x "$linux_out/$name/$name.x86_64"
    # Icons and desktop entry (applications menu, Steam Deck).
    mkdir -p "$linux_out/$name/icons"
    cp "$godot_dir"/icon/tyrian2000-*.png "$linux_out/$name/icons/"
    cp "$rewrite/packaging/linux/opentyrian2000-enhanced.desktop" "$rewrite/packaging/linux/install-desktop-entry.sh" \
        "$linux_out/$name/"
    chmod +x "$linux_out/$name/install-desktop-entry.sh"
    refuse_player_data "$linux_out/$name"
    (cd "$linux_out" && tar -czf "$release-linux-x86_64$suffix.tar.gz" "$name")
fi
# macOS (experimental, unsigned): the files shipped with the game go into
# Contents/Resources (godot/scripts/game_paths.gd); the player data never goes
# into the bundle (~/Library/Application Support/OpenTyrian2000 Enhanced).
mac_out="$OUT/macos$suffix"
if [ "$only" = macos ]; then
    app_name="OpenTyrian2000 Enhanced.app"
    app="$mac_out/$app_name"
    [ -f "$godot_dir/bin/libtyrian_godot.dylib" ] || { echo "libtyrian_godot.dylib missing (build-macos.sh)" >&2; exit 1; }
    rm -rf "$mac_out"
    mkdir -p "$mac_out"
    echo "== export \"macOS\" -> $app"
    run_godot_checked --headless --path "$godot_dir" --export-release "macOS" "$app"
    mac_exe="$app/Contents/MacOS/OpenTyrian2000 Enhanced"
    mac_lib="$app/Contents/Frameworks/libtyrian_godot.dylib"
    { [ -f "$mac_exe" ] && [ -f "$mac_lib" ]; } || { echo "incomplete bundle (executable or GDExtension missing): $app" >&2; exit 1; }
    # The Rust library's local symbols are stripped by build-macos.sh, before
    # the export signs it: Apple strip refuses the signed copy in the bundle.
    copy_data "$app/Contents/Resources"
    refuse_player_data "$app/Contents/Resources"
    # The files added after the export invalidate the signature made by Godot:
    # the whole bundle is signed again, ad-hoc (no Apple identity, not notarized).
    # Without a signature, Apple Silicon refuses to run the app at all.
    codesign --force --deep --sign - --entitlements "$rewrite/packaging/macos/entitlements.plist" "$app"
    codesign --verify --deep --strict --verbose=2 "$app"
    for bin in "$mac_exe" "$mac_lib"; do
        info="$(lipo -info "$bin")"
        echo "$info"
        for arch in x86_64 arm64; do
            [[ "$info" == *"$arch"* ]] || { echo "not universal, $arch missing: $bin" >&2; exit 1; }
        done
    done
    # ditto keeps the bundle's attributes and symbolic links (zip -r does not).
    (cd "$mac_out" && rm -f "$release-macos-universal$suffix.zip" \
        && ditto -c -k --keepParent "$app_name" "$release-macos-universal$suffix.zip")
fi

# HD pack: upscaled/ alone (same content as the full package) and its
# usage guide, common to Windows and Linux, to unzip into the game folder.
# The images are already compressed (PNG, WebP): zip -1 is enough.
if [ "$hd_pack" = 1 ]; then
    pack_dir="$OUT/hd-pack/stage"
    rm -rf "$OUT/hd-pack"
    mkdir -p "$pack_dir"
    copy_upscaled "$pack_dir"
    cp "$rewrite/packaging/HD-PACK-README.txt" "$pack_dir/"
    [ -d "$pack_dir/upscaled/sprites" ] || { echo "HD pack without upscaled/sprites" >&2; exit 1; }
    (cd "$pack_dir" && zip -qr -1 "../$release-hd-pack.zip" upscaled HD-PACK-README.txt)
    rm -rf "$pack_dir"
fi

echo "== packages"
find "$OUT" -maxdepth 2 -type f \( -name '*.zip' -o -name '*.tar.gz' \) -exec du -h {} \;
# "if" and not "[ -d ] && ...": with --only=windows, the missing linux folder
# made the script return the exit code 1 of that last test, although the package was built.
for d in "$win_out/$name" "$linux_out/$name" "$mac_out"; do
    if [ -d "$d" ]; then echo "$d:"; ls -la "$d"; fi
done
