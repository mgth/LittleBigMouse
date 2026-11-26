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

# The menu's entries: the UI's own pictograms (Start, Stop, Refresh, and the logo for
# Open), and Exit drawn here after lbm-ui's Icon::Power, the UI having none. Sizes: the
# small icon at 100, 125, 150, 200, 250 and 300 % (SM_CXSMICON).
for pair in open:lbm_logo start:Start stop:Stop refresh:Refresh exit:; do
    name=${pair%%:*} source=${pair#*:}
    file="$svg/$source.svg"
    [ -n "$source" ] || file="$here/menu/Exit.svg"
    for size in 16 20 24 32 40 48; do
        rsvg-convert -w "$size" -h "$size" "$file" -o "$here/menu/$name-$size.png"
    done
done
