# Little Big Mouse

<p align="center">
    <img src="https://raw.githubusercontent.com/mgth/LittleBigMouse/master/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm.png" width="200" alt="Little Big Mouse Logo"/>
</p>

> [!TIP]
> ## 🌐 New: [littlebigmouse.mgth.fr](https://littlebigmouse.mgth.fr/) — the official website
> Little Big Mouse now has a home of its own: **[littlebigmouse.mgth.fr](https://littlebigmouse.mgth.fr/)** — what it does, downloads, and guides, always in sync with this repository.
>
> ⚠️ Warning! Note that **littlebigmouse.com is not affiliated with this project**. It is used to distribute malware. We don't operate that site, don't know who does, and can't vouch for anything it publishes. The only official sources for Little Big Mouse are this repository — with its [Releases](https://github.com/mgth/LittleBigMouse/releases) page — and [littlebigmouse.mgth.fr](https://littlebigmouse.mgth.fr/).

> [!TIP]
> ## 🐧 New: Little Big Mouse is on the AUR
> Arch Linux users get a real package — no more cloning and building by hand:
>
> ```bash
> paru -S littlebigmouse   # or: yay -S littlebigmouse
> ```
>
> [![AUR version](https://img.shields.io/aur/version/littlebigmouse?logo=archlinux&logoColor=white&label=AUR)](https://aur.archlinux.org/packages/littlebigmouse)
> [![AUR last modified](https://img.shields.io/aur/last-modified/littlebigmouse?label=updated)](https://aur.archlinux.org/packages/littlebigmouse)
>
> It builds from the tagged sources and ships the desktop entry, the icon and the udev
> rule for `/dev/uinput`. One manual step remains — joining the `input` group — see
> [Linux](#linux-experimental).

> [!IMPORTANT]
> ## 🎉 5.6.0 is out!
> **Your monitors land where Windows has them.** The automatic placement lost the vertical offset between neighbouring screens, so a monitor sitting a little higher than the one beside it came out stacked a whole screen height away — and the first layout you were shown never matched what *Place from windows config* gave you afterwards (#531). Both are fixed, and the two directions now round-trip.
>
> **Border resistance, drawn where you want it** (#522): sections along any edge, each with its own resistance, and separate settings for moving and for dragging a window. **Live update** (#525) sends your changes to the mouse engine as you make them, so you can feel a layout before keeping it — pick it from the arrow next to the apply button. And when a layout traps the cursor somewhere you cannot click, **hold `Ctrl+Alt+Shift+M`** to get the mouse back (#528) — configurable in the options.
>
> Closing with unsaved changes now asks first (#530), and the C++ hook is gone: the Rust daemon is the only one, and the test suites finally run in CI (#524).
>
> **[⬇ Download 5.6.0](https://github.com/mgth/LittleBigMouse/releases/tag/v5.6.0)** — requires the [.NET 10 Runtime (x64)](https://dotnet.microsoft.com/en-us/download/dotnet/10.0).
>
> The rescue shortcut is Windows-only for now; the Linux release follows separately.
>
> Something not working? [Tell us](https://github.com/mgth/LittleBigMouse/issues) 🙏

Little Big Mouse (LBM) is an open-source software designed to enhance the multi-monitor experience on Windows 10 and 11 by providing accurate mouse screen crossover location within multi-DPI monitors environments. This is particularly useful for setups involving a 4K monitor alongside a full HD monitor.

## Donations

If you find Little Big Mouse helpful, consider supporting the project with a donation. Your contribution helps us maintain and improve the software.

[![Sponsor on GitHub](https://img.shields.io/badge/Sponsor-mgth-ea4aaa?logo=githubsponsors&logoColor=white)](https://github.com/sponsors/mgth)
[![ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/G5X022D1RW)
[![Donate using Liberapay](https://liberapay.com/assets/widgets/donate.svg)](https://liberapay.com/mgth/donate)

## Presentation Video

Check the very nice video from Touble Chute (a very big thanks to him):

[![TroubleChute](https://img.youtube.com/vi/6D46stJMP68/0.jpg)](https://www.youtube.com/watch?v=6D46stJMP68)


## Features

- **Smooth Mouse Transitions**: Ensures smooth and accurate mouse movement across screens with different DPI settings.
- **Screen Looping**: Lets the cursor wrap around the desktop — leave the last screen and re-enter from the first, horizontally or vertically.
- **Display Size Adjustments**: Allows for adjustments in the relative sizes of displays.
- **Border Resistance**: Allow some resistance before crossing.
- **Rescue Shortcut**: A keyboard way out when a layout leaves the cursor somewhere it cannot escape — the one recovery that does not need a click.
- **Live Update**: Feel a layout while you edit it. Changes go straight to the mouse engine as you make them, without being saved, so you can try a resistance or a position with the real mouse instead of applying it and undoing it.
- **Display Color and Brightness Balancing**: Offers control over color and brightness profiles of displays.
- **Access to Display Debugging Information**: Provides detailed information from your displays and drivers.

## Installation

1. Download the latest release from the [Releases](https://github.com/mgth/LittleBigMouse/releases) page.
2. Run the Little Big Mouse installer from your computer.
3. (Optional) Change the default installation path.
4. Follow the installation progress and complete the installation.
5. Access the program from the Start Menu.

## Linux (experimental)

The Linux port works, but is still experimental. Mouse routing uses an evdev/uinput
backend — it grabs the physical mice and drives a virtual pointer, exactly like the
Windows hook — with Wayland-portal and X11 fallbacks. It is developed and tested on
KDE Plasma 6 (Wayland); feedback from other desktops is very welcome on the
[issues](https://github.com/mgth/LittleBigMouse/issues) page.

### Arch Linux — install from the AUR

[![AUR version](https://img.shields.io/aur/version/littlebigmouse?logo=archlinux&logoColor=white&label=AUR)](https://aur.archlinux.org/packages/littlebigmouse)

```bash
paru -S littlebigmouse   # or: yay -S littlebigmouse
```

That is the whole install. The package builds the Rust hook daemon and the Avalonia UI
from the tagged sources, and lays down:

- the app in `/usr/lib/littlebigmouse` with `littlebigmouse` on your `PATH`;
- a desktop entry and the icon, so it shows up in your launcher;
- a udev rule granting the `input` group write access to `/dev/uinput`.

It depends on `dotnet-runtime-10.0` rather than bundling .NET, so runtime security
updates come through pacman. `ddcutil`, `kscreen` and `argyllcms` are optional
dependencies — install them for monitor brightness/contrast control, Plasma layout
detection and colorimeter support respectively.

**One manual step remains**, because a package cannot add you to a group:

```bash
sudo gpasswd -a "$USER" input   # then log out and back in
```

Without it the daemon falls back to the Wayland portal or X11, which cannot intercept
the cursor crossing between adjacent screens on KWin. Note that `input` group membership
lets every program you run read all keyboard and mouse events.

The packaging recipe lives in [`packaging/arch`](packaging/arch); issues with the
package go to the [issue tracker](https://github.com/mgth/LittleBigMouse/issues) like
anything else.

### Other distributions — build from source

- The **.NET 10 SDK** (the runtime alone is not enough to build) and the **Rust
  toolchain** (`cargo`):
  - Arch / CachyOS: `sudo pacman -S dotnet-sdk rust`
  - other distros: [.NET SDK](https://dotnet.microsoft.com/download/dotnet/10.0) and [rustup](https://rustup.rs)
- Access to `/dev/input` and `/dev/uinput` for the evdev backend: add yourself to the
  `input` group (`sudo usermod -aG input $USER`), then **log out and back in** — without
  it the daemon silently falls back to the portal/X11 backend.

### Build and run

```bash
git clone --recursive https://github.com/mgth/LittleBigMouse.git
cd LittleBigMouse
./run-lbm.sh
```

`run-lbm.sh` builds the hook daemon (cargo) and the UI (dotnet), stages everything and
relaunches the app. UI and daemon logs land in `/tmp/lbm-ui.log`. If you cloned without
`--recursive`, run `git submodule update --init --recursive` first — the HLab projects
are submodules.

### Tests

```bash
dotnet test LittleBigMouse.sln
cargo test --manifest-path LittleBigMouse-Hook-Rust/Cargo.toml
```

Both are what CI runs, before it builds anything for shipping.

## Usage

Documentation : https://littlebigmouse.mgth.fr/docs/ and https://github.com/mgth/LittleBigMouse/wiki

Little Big Mouse provides a single-window interface with three main sections:

- **Top Panel**: Access view tabs for display and display adapter information, changing relative sizes and positions of displays, and adjusting color and brightness profiles.
- **Center Panel**: Displays information about your display devices, including makes and models, capabilities, adapters, and relative positions.
- **Bottom Panel**: Exporting or copying the layout, and the operations that act on it — save, apply, stop, undo.

### Applying a layout

The apply button hands the layout to the mouse engine and saves it. The arrow next to it
chooses *how* changes get there:

- **Apply on click** — the default. Nothing reaches the engine until you press the button.
- **Live update** — every change reaches the running engine as you make it, so the layout
  can be felt with the real mouse straight away.

Live update saves nothing: the button is still what keeps a layout, and **Undo** goes back
to the last saved one. Nothing about it survives closing the app either — a restarted
engine comes back to the layout you last applied, never to something you were only trying
out.

### If the cursor gets trapped

A layout can leave the cursor somewhere it cannot get out of — and then Undo, Stop and the
tray menu are all behind a click you cannot make. **Hold `Ctrl+Alt+Shift+M` for about a
second** and the mouse comes back:

- while **live update** is on, the layout you were trying is thrown away and the engine
  goes back to the one you saved;
- otherwise the engine stops, and the mouse behaves the way Windows alone would until you
  start it again.

The combination is yours to change under **Options → Mouse → Rescue shortcut**: click it
and press the one you want. If another application already owns that combination, the
setting says so — a rescue you only discover is missing when you need it would be worse
than none.

Windows only for now — the setting is not shown elsewhere. Wayland has no global
shortcut without a portal, and reading one from evdev would mean opening the keyboard
devices, which is a great deal more than a mouse utility should ask for.

## Support

If you encounter any issues or have suggestions for improvements, please open an issue on our [Issues](https://github.com/mgth/LittleBigMouse/issues) page. We appreciate your feedback and are always looking to improve the tool.

**We Welcome Contributions**: Your help is invaluable! Whether it's reporting bugs, suggesting new features, or submitting pull requests, we encourage you to get involved. Your contributions can make a significant difference in the development and improvement of Little Big Mouse. **First time?** Check out [This guide](https://github.com/firstcontributions/first-contributions) to get started.

## Acknowledgements

- **HLab Project**: Little Big Mouse depends on the HLab project for its functionality. Check out the [HLab](https://github.com/mgth/HLab.Core) and [HLab.Avalonia](https://github.com/mgth/HLab.Avalonia) repositories for more information.
- **MouseKeyboardActivityMonitor**: Inspired by the [MouseKeyboardActivityMonitor](https://github.com/gmamaladze/globalmousekeyhook) project.
- **Task Scheduler Managed Wrapper**: Utilizes the [Task Scheduler Managed Wrapper](https://github.com/dahall/TaskScheduler) for scheduling tasks.

Thank you for using Little Big Mouse!
