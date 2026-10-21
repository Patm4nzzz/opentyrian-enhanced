#!/usr/bin/env bash
# Adds (or removes with --remove) the game launcher in the user's
# applications menu: .desktop entry and Tyrian icons, without administrator
# rights. The game stays in this folder; the launcher points to it.
set -euo pipefail

dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
data="${XDG_DATA_HOME:-$HOME/.local/share}"
desktop_file="$data/applications/opentyrian2000-enhanced.desktop"

if [ "${1:-}" = "--remove" ]; then
    rm -f "$desktop_file" "$data"/icons/hicolor/*/apps/opentyrian2000-enhanced.png
    echo "Launcher removed."
    exit 0
fi

mkdir -p "$data/applications"
sed "s|@INSTALL_DIR@|$dir|g" "$dir/opentyrian2000-enhanced.desktop" >"$desktop_file"
for icon in "$dir"/icons/tyrian2000-*.png; do
    size="$(basename "$icon" .png)"
    size="${size#tyrian2000-}"
    mkdir -p "$data/icons/hicolor/${size}x${size}/apps"
    cp "$icon" "$data/icons/hicolor/${size}x${size}/apps/opentyrian2000-enhanced.png"
done
echo "Launcher installed: $desktop_file"
