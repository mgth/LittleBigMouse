# Little Big Mouse

<p align="center">
    <img src="https://raw.githubusercontent.com/mgth/LittleBigMouse/master/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/lbm.png" width="200" alt="Little Big Mouse Logo"/>
</p>

> [!IMPORTANT]
> ## 🎉 5.4.1 is out!
> Screen **rotation** works again, after being broken since the v5 rewrite (#507): portrait monitors are drawn portrait, every orientation keeps its own saved layout, and driver-side rotations are detected. On top of the 5.4.0 base: the mouse engine now runs on a **memory-safe Rust daemon**, sleep/resume hardening, Xbox / Game Pass games excluded by default, per-model or per-monitor border settings, and tray icon fixes.
>
> **[⬇ Download 5.4.1](https://github.com/mgth/LittleBigMouse/releases/tag/v5.4.1)** — requires the [.NET 10 Runtime (x64)](https://dotnet.microsoft.com/en-us/download/dotnet/10.0).
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
- **DPI Aware Mouse Movement**: Adjusts mouse speed to remain consistent across monitors with different DPI values.
- **Infinite Mouse Scrolling**: Enables seamless cursor movement between screens, either horizontally or vertically.
- **Display Size Adjustments**: Allows for adjustments in the relative sizes of displays.
- **Border Resistance**: Allow some resistance before crossing.
- **Display Color and Brightness Balancing**: Offers control over color and brightness profiles of displays.
- **Access to Display Debugging Information**: Provides detailed information from your displays and drivers.

## Installation

1. Download the latest release from the [Releases](https://github.com/mgth/LittleBigMouse/releases) page.
2. Run the Little Big Mouse installer from your computer.
3. (Optional) Change the default installation path.
4. Follow the installation progress and complete the installation.
5. Access the program from the Start Menu.

## Linux (experimental)

The Linux port works, but is still experimental and currently builds from source only.
Mouse routing uses an evdev/uinput backend — it grabs the physical mice and drives a
virtual pointer, exactly like the Windows hook — with Wayland-portal and X11 fallbacks.
It is developed and tested on KDE Plasma 6 (Wayland); feedback from other desktops is
very welcome on the [issues](https://github.com/mgth/LittleBigMouse/issues) page.

### Prerequisites

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

## Usage

Documentation : https://github.com/mgth/LittleBigMouse/wiki

Little Big Mouse provides a single-window interface with three main sections:

- **Top Panel**: Access view tabs for display and display adapter information, changing relative sizes and positions of displays, and adjusting color and brightness profiles.
- **Center Panel**: Displays information about your display devices, including makes and models, capabilities, adapters, and relative positions.
- **Bottom Panel**: Offers options and operations, including copying config to clipboard, enabling/disabling LBM functionality, and adjusting speed, pointer size, and corner crossing.

## Support

If you encounter any issues or have suggestions for improvements, please open an issue on our [Issues](https://github.com/mgth/LittleBigMouse/issues) page. We appreciate your feedback and are always looking to improve the tool.

Little Big Mouse does not collect analytics or telemetry. See the [privacy policy](PRIVACY.md) for the limited network features and local data handling. Release signing and verification are documented in [SIGNING.md](SIGNING.md).

**We Welcome Contributions**: Your help is invaluable! Whether it's reporting bugs, suggesting new features, or submitting pull requests, we encourage you to get involved. Your contributions can make a significant difference in the development and improvement of Little Big Mouse. **First time?** Check out [This guide](https://github.com/firstcontributions/first-contributions) to get started.

## Acknowledgements

- **HLab Project**: Little Big Mouse depends on the HLab project for its functionality. Check out the [HLab](https://github.com/mgth/HLab.Core) and [HLab.Avalonia](https://github.com/mgth/HLab.Avalonia) repositories for more information.
- **MouseKeyboardActivityMonitor**: Inspired by the [MouseKeyboardActivityMonitor](https://github.com/gmamaladze/globalmousekeyhook) project.
- **Task Scheduler Managed Wrapper**: Utilizes the [Task Scheduler Managed Wrapper](https://github.com/dahall/TaskScheduler) for scheduling tasks.
- **SignPath Foundation**: Free code signing provided by [SignPath.io](https://signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

Thank you for using Little Big Mouse!
