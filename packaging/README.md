# Linux packaging

Distribution-agnostic assets live in `linux/`; per-distribution recipes get their
own directory (`arch/` for now).

| File | Installed as |
| --- | --- |
| `linux/littlebigmouse.desktop` | `/usr/share/applications/littlebigmouse.desktop` |
| `linux/99-littlebigmouse-uinput.rules` | `/usr/lib/udev/rules.d/99-littlebigmouse-uinput.rules` |
| `LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/lbm_logo.svg` | `/usr/share/icons/hicolor/scalable/apps/littlebigmouse.svg` |

## Install layout

Everything the app ships goes into a single directory:

```
/usr/lib/littlebigmouse/
    LittleBigMouse.Ui.Avalonia      app host (framework-dependent, net10.0)
    lbm-hook                        routing daemon (Rust)
    lbm-pattern                     native-Wayland test pattern viewer
    *.dll, libSkiaSharp.so, ...
/usr/bin/littlebigmouse -> ../lib/littlebigmouse/LittleBigMouse.Ui.Avalonia
```

This is not cosmetic: `LittleBigMouseClientService.FindHookPath` looks for
`lbm-hook` next to `AppContext.BaseDirectory`, and `WaylandPattern.FindHelper`
does the same for `lbm-pattern`. Neither has a system-path fallback nor an
environment override, so the three binaries must stay siblings.

The `/usr/bin` entry is a symlink rather than a wrapper script on purpose. The
.NET app host resolves its own location through `/proc/self/exe`, which follows
symlinks, so `AppContext.BaseDirectory` is still `/usr/lib/littlebigmouse/`.
The daemon's "was I launched by the UI?" test (`parent_process_path()` in
`src/platform/linux/process.rs`, which reads the already-resolved
`/proc/<ppid>/exe` and looks for `LittleBigMouse` in it) also keeps working —
otherwise the daemon would start in standalone auto-run mode.

Per-user state stays where `LbmPaths` puts it: `~/.config/LittleBigMouse/` and
`~/.local/share/LittleBigMouse/`.

## Arch Linux / AUR

`arch/` holds the exact content of the `littlebigmouse` AUR repository
(`PKGBUILD`, `.SRCINFO`, `littlebigmouse.install`).

`pkgver` must always name a tag that already contains this `packaging/` directory:
`package()` installs the `.desktop` entry and the udev rule from the checkout, not
from the AUR repository.

### First publication

```sh
git clone ssh://aur@aur.archlinux.org/littlebigmouse.git aur-littlebigmouse
cp packaging/arch/{PKGBUILD,.SRCINFO,littlebigmouse.install} aur-littlebigmouse/
cd aur-littlebigmouse && git add -A && git commit -m 'Initial import: littlebigmouse 5.5.2' && git push
```

The AUR account needs an SSH key registered at
<https://aur.archlinux.org/account/>; the repository is created by the first push.

### Every release

```sh
cd packaging/arch
# 1. bump pkgver to the new tag (and reset pkgrel to 1)
# 2. regenerate the metadata
makepkg --printsrcinfo > .SRCINFO
# 3. build it once from scratch, ideally in a clean chroot
makepkg -f            # or: extra-x86_64-build
# 4. copy the three files to the AUR clone, commit, push
```

Notes:

* The GitHub release tarball is **not** usable as a source: it does not carry the
  `HLab.Core` / `HLab.Avalonia` submodules. The PKGBUILD clones the tag plus both
  submodule repositories and wires them together in `prepare()`.
* The package is framework-dependent (`depends=dotnet-runtime`), so .NET security
  updates do not require a rebuild.
* Nothing had ever run `dotnet publish` on this solution: the Layout and Vcp
  plugins each copied an identical, unused `Assets/Icon/colors.xml` to the output,
  which `dotnet publish` rejects with NETSDK1152 (`dotnet build`, which the Windows
  CI uses, tolerates it). The `<Content>` items are gone from both csproj files;
  `-p:ErrorOnDuplicatePublishOutputFiles=false` stays in the PKGBUILD as a guard.
* The in-app updater downloads the Windows installer from the GitHub releases and
  executes it, so it is disabled off Windows (`IApplicationUpdater.IsSupported`) —
  a packaged application must not update itself behind the package manager's back.

## Input permissions

The default Linux backend grabs the physical mice (`EVIOCGRAB` on
`/dev/input/event*`) and re-injects a corrected stream through a uinput virtual
pointer. That needs read access to `/dev/input/event*` (membership of the `input`
group) and write access to `/dev/uinput` (the shipped udev rule). Without both,
the daemon falls back to the XDG InputCapture portal or X11, which cannot
intercept crossings between adjacent outputs on KWin.

Adding a user to a group cannot be done from a package, so the Arch package
prints the `gpasswd -a "$USER" input` instruction on install.
