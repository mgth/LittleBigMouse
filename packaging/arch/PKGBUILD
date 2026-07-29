# Maintainer: Mathieu Grenet <mathieu@mgth.fr>

pkgname=littlebigmouse
# Must be a tag that already contains packaging/linux (the .desktop entry and the
# udev rule are taken from the checkout, not carried by the AUR repository).
pkgver=5.5.2
pkgrel=1
pkgdesc="Seamless mouse travel between monitors of different sizes, resolutions and DPI"
arch=('x86_64')
url="https://github.com/mgth/LittleBigMouse"
license=('GPL-3.0-only')
depends=(
  # Exact major: a net10.0 framework-dependent app does not roll forward onto a
  # future .NET 11 runtime. Satisfied by either dotnet-runtime or dotnet-runtime-10.0.
  'dotnet-runtime-10.0'
  'fontconfig'
  'gcc-libs'
  'glibc'
  'libice'
  'libsm'
  'libx11'
  'libxcursor'
  'libxext'
  'libxi'
  'libxrandr'
)
# dotnet-sdk pulls dotnet-targeting-pack, which the net10.0 build needs.
makedepends=('dotnet-sdk-10.0' 'cargo' 'git')
optdepends=(
  'kscreen: display layout detection and "apply layout to system" on KDE Plasma'
  'xorg-xrandr: display layout fallback outside Plasma'
  'ddcutil: DDC/CI monitor control (brightness, contrast, input source)'
  'argyllcms: colorimeter support in the monitor calibration panel'
  'libglvnd: GPU-accelerated rendering'
)
install="$pkgname.install"
# The GitHub release tarballs do not carry the HLab.Core / HLab.Avalonia
# submodules, so the sources are cloned separately and wired in prepare().
# Full (non-shallow) clones: the superproject pins exact submodule commits.
source=(
  "$pkgname::git+$url.git#tag=v$pkgver"
  "HLab.Core::git+https://github.com/mgth/HLab.Core.git"
  "HLab.Avalonia::git+https://github.com/mgth/HLab.Avalonia.git"
)
sha256sums=('SKIP'
            'SKIP'
            'SKIP')

_ui_project='LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/LittleBigMouse.Ui.Avalonia.csproj'

prepare() {
  cd "$srcdir/$pkgname"

  git submodule init
  git config submodule.HLab.Core.url "$srcdir/HLab.Core"
  git config submodule.HLab.Avalonia.url "$srcdir/HLab.Avalonia"
  git -c protocol.file.allow=always submodule update

  # Keep NuGet and cargo inside $srcdir: no writes to the packager's ~/.nuget or
  # ~/.cargo, and the download step stays here in prepare().
  export DOTNET_CLI_TELEMETRY_OPTOUT=1
  export DOTNET_NOLOGO=1
  export NUGET_PACKAGES="$srcdir/nuget"
  dotnet restore "$_ui_project" -r linux-x64

  cd "$srcdir/$pkgname/LittleBigMouse-Hook-Rust"
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
  cd "$srcdir/$pkgname"

  export DOTNET_CLI_TELEMETRY_OPTOUT=1
  export DOTNET_NOLOGO=1
  export NUGET_PACKAGES="$srcdir/nuget"

  # Framework-dependent: the runtime comes from the dotnet-runtime package, so
  # .NET security updates do not need a rebuild of this one.
  # -f net10.0 is required (the project declares <TargetFrameworks>, plural).
  dotnet publish "$_ui_project" \
    --no-restore \
    -c Release \
    -f net10.0 \
    -r linux-x64 \
    --self-contained false \
    -p:Version="$pkgver" \
    -p:DebugType=none \
    -o "$srcdir/publish"

  cd "$srcdir/$pkgname/LittleBigMouse-Hook-Rust"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  # lbm-hook: the routing daemon. lbm-pattern: native-Wayland fullscreen test
  # pattern viewer used by the monitor calibration panel.
  cargo build --frozen --release --bin lbm-hook --bin lbm-pattern
}

check() {
  cd "$srcdir/$pkgname/LittleBigMouse-Hook-Rust"
  export RUSTUP_TOOLCHAIN=stable
  export CARGO_TARGET_DIR=target
  cargo test --frozen --release
}

package() {
  cd "$srcdir/$pkgname"

  # The app host locates lbm-hook / lbm-pattern next to itself
  # (AppContext.BaseDirectory), so the whole deployment lives in one directory.
  install -dm755 "$pkgdir/usr/lib/$pkgname"
  cp -a "$srcdir/publish/." "$pkgdir/usr/lib/$pkgname/"
  chmod -R u=rwX,go=rX "$pkgdir/usr/lib/$pkgname"
  chmod 755 "$pkgdir/usr/lib/$pkgname/LittleBigMouse.Ui.Avalonia"

  install -Dm755 LittleBigMouse-Hook-Rust/target/release/lbm-hook \
    "$pkgdir/usr/lib/$pkgname/lbm-hook"
  install -Dm755 LittleBigMouse-Hook-Rust/target/release/lbm-pattern \
    "$pkgdir/usr/lib/$pkgname/lbm-pattern"

  # A symlink is enough: the .NET app host resolves its own location through
  # /proc/self/exe, which follows symlinks, so AppContext.BaseDirectory is still
  # /usr/lib/littlebigmouse and the sibling lookup for lbm-hook holds. It also
  # keeps the daemon's "launched by the UI" check working (it reads the resolved
  # /proc/<ppid>/exe and looks for "LittleBigMouse" in it).
  install -dm755 "$pkgdir/usr/bin"
  ln -s "../lib/$pkgname/LittleBigMouse.Ui.Avalonia" "$pkgdir/usr/bin/$pkgname"

  install -Dm644 packaging/linux/littlebigmouse.desktop \
    "$pkgdir/usr/share/applications/$pkgname.desktop"
  install -Dm644 LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia/Assets/Icon/lbm_logo.svg \
    "$pkgdir/usr/share/icons/hicolor/scalable/apps/$pkgname.svg"
  install -Dm644 packaging/linux/99-littlebigmouse-uinput.rules \
    "$pkgdir/usr/lib/udev/rules.d/99-$pkgname-uinput.rules"

  install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
  install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
}
