# macOS (experimental)

Little Big Mouse on macOS uses the same Avalonia UI and the same Rust traversal
engine as Windows and Linux. Only the edges that touch the OS are macOS-specific:

| Piece | Where | What it does |
|---|---|---|
| Hook daemon backend | `rust/crates/lbm-hook/src/hook/macos/` | A CoreGraphics event tap that reads every mouse move, runs it through the engine, and moves the cursor with `CGWarpMouseCursorPosition` when it crosses a border |
| Daemon platform layer | `rust/crates/lbm-hook/src/platform/macos/` | The cursor environment (position, warp, emulated clip, Ctrl and button state), the display list, process paths |
| UI platform layer | `LittleBigMouse.Platform.MacOS/` | Display discovery through CoreGraphics (bounds, physical size in mm, scale, rotation), display arrangement, the info panel |

Persistence, the monitor-to-model mapping and the per-user data directories are
the Linux ones, shared rather than copied: settings in `~/.config/LittleBigMouse`,
runtime data and the daemon's socket in `~/.local/share/LittleBigMouse`.

## Build and run

**One click:** double-click `Run LittleBigMouse (Mac).command` in Finder. It
installs what's missing (Xcode Command Line Tools, Rust, the .NET 10 SDK), fetches
the submodules, pulls the latest commits of the branch, builds, launches, and on
the first run opens the Accessibility settings. The build log is written to
`~/Desktop/LittleBigMouse-build.log`.

**By hand:**

Prerequisites:

- Xcode Command Line Tools (`xcode-select --install`)
- Rust through [rustup](https://rustup.rs). The repository pins its toolchain in
  `rust-toolchain.toml`, and rustup installs it on the first build.
- The .NET 10 SDK

```sh
git clone --recursive https://github.com/alextirgard00n/LittleBigMouse-Mac
cd LittleBigMouse-Mac
git checkout macos-port
./run-lbm.sh
```

`run-lbm.sh` builds the daemon (`cargo build -p lbm-hook`) and the UI, puts the
daemon next to the UI, and launches it. The log goes to `$TMPDIR/lbm-ui.log`.

To build the pieces yourself:

```sh
cargo build --release -p lbm-hook --manifest-path rust/Cargo.toml
dotnet build -c Release LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia
```

## Accessibility permission

The daemon needs permission to watch and move the cursor. macOS asks for it the
first time the engine starts. Grant it in **System Settings → Privacy & Security →
Accessibility**.

macOS gives the permission to the *responsible* app, not to `lbm-hook` itself.
When you launch from `run-lbm.sh`, that is your terminal (Terminal, iTerm, …), so
the terminal is the entry to enable. The daemon retries every few seconds, so
you don't have to restart after granting it. If nothing happens, stop the engine
and start it again from the UI.

If the entry is on but the engine still doesn't start (this can happen after a
rebuild changes the binary), remove the entry with **−**, then add it again.

## How it maps to the Windows version

- **Coordinates.** The engine works in CoreGraphics global *points*, the space
  macOS moves the cursor in. On a Retina panel one point is two pixels. The
  zones' millimetre geometry, which is what makes crossings line up physically,
  is the same as on the other platforms.
- **Free edges.** macOS keeps the cursor on a display. If you push into an edge
  that has no neighbouring display in System Settings, the cursor stops there.
  Windows would report a position past the edge, and the engine uses that to
  cross and to use up border resistance. The tap rebuilds that position from the
  event's motion delta, so crossings toward displays that aren't touching in
  System Settings work as they do on Windows.
- **Clipping.** macOS has no `ClipCursor`. Holding the cursor at a border
  (resistance, or where no display continues) is done in software: the cursor is
  pulled back into the current display as soon as the system moves it out.
- **Ctrl to cross.** The Ctrl override for border resistance reads the hardware
  Ctrl key, as on Windows.

## Known gaps

- **Process exclusion** (pausing while a game or another listed app is in front)
  isn't wired yet: it needs AppKit to know the frontmost app. A game that detaches
  the cursor from the mouse is detected and left alone.
- **Rescue shortcut** isn't implemented (also missing on Linux). Ctrl still forces
  a crossing, and stopping the engine releases everything.
- **Wallpaper spanning** isn't available, so the plugin hides itself.
- **DDC/CI monitor controls** (brightness and so on) aren't available.
- **Display enable/disable** from the UI is not supported: macOS has no public
  API for it.
- **Scale changes** from the layout editor aren't applied. On macOS, scaling is a
  display-mode choice in System Settings.
- **Reported sizes.** Some external monitors report a wrong physical size through
  CoreGraphics. Correct it in the size editor, the same as on Windows. Rotated
  (portrait) displays are the first thing to check if a size looks transposed.

## Test checklist

1. The UI opens and shows every display at its real size and position.
2. Start the engine. The Accessibility prompt appears. After you grant it, the
   UI shows the engine running.
3. Move slowly across each shared edge. The cursor comes out at the same
   physical height on the other display, with no jump.
4. Move across an edge at speed. There's no hitch or pause right after the
   crossing.
5. Drag a window across a border. It follows the cursor, and the drag
   resistance setting applies.
6. Set some border resistance. The cursor holds at the edge until you push
   through, or hold Ctrl.
7. Unplug or rearrange a display in System Settings. The UI rebuilds and the
   engine restarts on the new layout.
8. Stop the engine. Cursor movement is back to normal macOS behaviour.
