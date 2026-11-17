# Little Big Mouse for macOS

<p align="center">
    <img src="https://raw.githubusercontent.com/mgth/LittleBigMouse/master/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm.png" width="200" alt="Little Big Mouse Logo"/>
</p>

A macOS port of **[Little Big Mouse](https://github.com/mgth/LittleBigMouse)** by
Mathieu Grenet, the open-source tool that makes the mouse cross between monitors of
different sizes and pixel densities as if they were one physical surface.

Put a 27" 4K screen next to a 16" portable monitor, and macOS moves the cursor by
points, not millimetres: crossing from one to the other, the cursor jumps up or down,
and some parts of an edge lead nowhere. Little Big Mouse learns the real size and
position of every screen, and moves the cursor across a border to the same
*physical* spot on the other side.

This fork adds macOS support to the original project, which runs on Windows and
Linux. The mouse engine, the layout editor and the settings are the original ones;
what's new is the macOS layer underneath them (see [How the port works](#how-the-port-works)).

> [!NOTE]
> The only official sources for the original Little Big Mouse are
> [mgth/LittleBigMouse](https://github.com/mgth/LittleBigMouse) and
> [littlebigmouse.mgth.fr](https://littlebigmouse.mgth.fr/).
> **littlebigmouse.com is not affiliated with the project** and has been used to
> distribute malware.

## Quick start

**Requirements:** a Mac on a recent macOS (Apple Silicon or Intel). The launcher
installs everything else.

> [!IMPORTANT]
> Experimental. So far it has been used on one two-monitor setup (a 27" 4K OLED with
> a 16" portable monitor in portrait). Reports from other setups are welcome.

1. Clone the repository with its submodules:
   ```sh
   git clone --recursive https://github.com/alextirgard00n/LittleBigMouse-Mac
   ```
2. In Finder, double-click **`Run LittleBigMouse (Mac).command`**.

The launcher:
- installs what's missing: the Xcode Command Line Tools, Rust and the .NET SDK
  (through Homebrew if you have it, which may ask for your password);
- pulls the latest commits and the submodules;
- builds the mouse hook and the app (the first build takes a few minutes), and
  writes the build log to `~/Desktop/LittleBigMouse-build.log`;
- launches Little Big Mouse;
- on the first run, opens **System Settings → Privacy & Security → Accessibility**.

### Accessibility permission

The mouse hook needs Accessibility access to watch and move the cursor. macOS gives
that permission to the app that *launched* Little Big Mouse, which is **Terminal**
when you use the launcher. Switch Terminal on in the list (add it with **+** if it
isn't there), then press **Start** in Little Big Mouse. If the engine was already
started, it picks up the permission within a few seconds.

If Terminal is switched on but the engine still won't start (this can happen after a
rebuild), remove Terminal from the list with **−** and add it again.

## Setting up your layout

The app opens on the layout view, one frame per monitor. Getting these right is what
makes the cursor land where you expect.

**Size.** Each monitor's visible screen area, in mm. macOS asks each monitor for its
size, but some monitors (portable ones especially) report nonsense. If a screen's
DPI looks far off, measure the lit area with a ruler and enter it. Typical sizes:

| Screen | Visible area (W × H) |
|---|---|
| 27" 16:9 (4K or 1440p) | 597 × 336 mm |
| 16" 16:10, landscape | 345 × 215 mm |
| 16" 16:10, portrait | 215 × 345 mm |
| 24" 16:10, landscape | 518 × 324 mm |

**Percentage.** Viewing-distance compensation: lower it for a monitor that sits
farther from you than the others, so the cursor moves as the screen *looks* from your
chair. For screens at the same distance, leave it at 100%. (It also shrinks a
monitor whose reported size is too big. That works, but entering the real size is
cleaner and keeps the DPI readout honest.)

**Position.** Drag the frames so they match your desk, especially how high each screen
sits relative to its neighbour. That vertical offset decides where the cursor comes
out when it crosses.

**Borders.** The bezel around each screen's lit area, in mm. With bezels entered, the
cursor "travels" across the dead space between two screens, so diagonal moves stay
straight. Only the sides that face another monitor matter. If the monitors are apart
on the desk, add that gap too.

**Border resistance.** Optional: the cursor pauses at an edge before crossing, so you
don't fall onto the other screen by accident. It can be set separately for dragging a
window. Hold **Ctrl** to cross straight through.

Press **Apply** (the play button at the bottom) to send the layout to the engine and
save it. The arrow next to it switches to **live update**, where every change is felt
with the real mouse as you make it, without being saved.

## Everyday use

- **Start / stop:** the play and stop buttons at the bottom of the window.
- **Update:** double-click the launcher again. It pulls the latest commits, rebuilds
  only what changed and relaunches. (It skips the pull if you have local changes.)
- **Logs:** the app log is `$TMPDIR/lbm-ui.log`. If the app quits right after
  starting, the launcher shows the end of that log and copies it to your Desktop.
- **Settings** live in `~/.config/LittleBigMouse`; runtime files in
  `~/.local/share/LittleBigMouse`.

## What works on macOS

| Feature | macOS |
|---|---|
| Physical-size crossing between monitors | ✅ |
| Corner crossing, screen looping | ✅ |
| Border resistance (moving and dragging), Ctrl to force | ✅ |
| Monitor sizes, bezels, distance compensation | ✅ |
| Rotated (portrait) monitors | ✅ (check the size it reports) |
| Rearranging displays and choosing the main display from the app | ✅ |
| Re-detecting when a display is plugged, unplugged or moved | ✅ |
| Pausing while a chosen app (a game) is in front | ❌ not yet |
| Rescue shortcut (Ctrl+Alt+Shift+M) | ❌ not yet (also missing on Linux) |
| Wallpaper spanning | ❌ not yet |
| Brightness / colour (DDC/CI) controls | ❌ not yet |
| Turning displays on/off from the app | ❌ no public macOS API |
| Changing scale from the layout editor | ❌ set in System Settings → Displays |

If the cursor ever gets stuck somewhere, stopping the engine (or quitting the app)
gives it back to macOS straight away.

## How the port works

| Piece | Where | Role |
|---|---|---|
| Mouse hook | `rust/crates/lbm-hook/src/hook/macos/` | A CoreGraphics event tap that sees every mouse move, runs it through the shared engine, and moves the cursor with `CGWarpMouseCursorPosition` when it crosses a border |
| Hook platform layer | `rust/crates/lbm-hook/src/platform/macos/` | Cursor position and warp, the emulated clip, Ctrl and button state, the display list |
| App platform layer | `LittleBigMouse.Platform.MacOS/` | Display discovery and arrangement through CoreGraphics; reuses the Linux layout mapping and JSON settings |
| Launcher | `Run LittleBigMouse (Mac).command` | One-click setup, build and launch |

Three things work differently from Windows under the hood:
- **Coordinates** are CoreGraphics points, the space macOS moves the cursor in (two
  pixels per point on a Retina screen). The physical-size maths is unchanged.
- **Free edges:** macOS stops the cursor at an edge that has no neighbour in System
  Settings. The hook rebuilds where the cursor *would* have gone from the mouse's
  motion, so crossings toward screens that don't touch in System Settings still work.
- **No `ClipCursor`:** holding the cursor at a border is done in software, pulling it
  back as soon as macOS lets it out.

More detail, and a test checklist, in [docs/macos.md](docs/macos.md).

## Building by hand

```sh
git submodule update --init --recursive
cargo build --release -p lbm-hook --manifest-path rust/Cargo.toml
dotnet build -c Release LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia
./run-lbm.sh --no-build
```

You need the Xcode Command Line Tools, Rust ([rustup](https://rustup.rs); the exact
toolchain is pinned in `rust-toolchain.toml`) and a .NET SDK, 10 or newer. The app
targets .NET 10 and runs on a newer runtime when 10 isn't installed.

Tests for the platform-neutral Rust crates:
```sh
cargo test --manifest-path rust/Cargo.toml -p lbm-geom -p lbm-zones -p lbm-engine -p lbm-ipc
```

## Windows and Linux

This fork keeps the Windows and Linux code of the original project, but it's focused
on macOS. On Windows and Linux, use the original:
**[mgth/LittleBigMouse](https://github.com/mgth/LittleBigMouse)**, with its
[releases](https://github.com/mgth/LittleBigMouse/releases),
[documentation](https://littlebigmouse.mgth.fr/docs/) and the
[AUR package](https://aur.archlinux.org/packages/littlebigmouse) for Arch Linux.

## Support

Problems with the macOS port: open an issue on
[this repository](https://github.com/alextirgard00n/LittleBigMouse-Mac/issues),
with the build or app log if there is one. Everything else belongs upstream at
[mgth/LittleBigMouse](https://github.com/mgth/LittleBigMouse/issues).

Little Big Mouse collects no analytics and sends no telemetry; see the
[privacy policy](PRIVACY.md).

## Credits

- **Little Big Mouse** is the work of [Mathieu Grenet](https://github.com/mgth) and its
  contributors. If you find it useful, consider supporting the original project:

  [![Sponsor on GitHub](https://img.shields.io/badge/Sponsor-mgth-ea4aaa?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/mgth)
  [![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/G5X022D1RW)
  [![Donate using Liberapay](https://liberapay.com/assets/widgets/donate.svg)](https://liberapay.com/mgth/donate)
- **macOS port** by [Alex Tirgardoon](https://github.com/alextirgard00n).
- **HLab**: the [HLab](https://github.com/mgth/HLab.Core) and
  [HLab.Avalonia](https://github.com/mgth/HLab.Avalonia) libraries the app is built on.
- Inspired in part by
  [MouseKeyboardActivityMonitor](https://github.com/gmamaladze/globalmousekeyhook).

## License

GPL-3.0, like the original project. See [LICENSE](LICENSE).
