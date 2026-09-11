#!/bin/sh
# The tray icons: the UI's SVGs (the C# tray's), rendered at the sizes trays ask for.
# Rerun after an SVG changes; needs rsvg-convert (librsvg).
set -eu
here=$(cd "$(dirname "$0")" && pwd)
svg="$here/../../../../LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon"
for state in on off dead paused; do
    for size in 16 22 24 32 48 64; do
        rsvg-convert -w "$size" -h "$size" "$svg/lbm_$state.svg" -o "$here/lbm_$state-$size.png"
    done
done
