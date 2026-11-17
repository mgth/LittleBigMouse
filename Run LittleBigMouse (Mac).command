#!/usr/bin/env bash
#
# Run LittleBigMouse (Mac).command — double-click in Finder to set up, build and
# launch LittleBigMouse on macOS.
#
# First run: installs what's missing (Xcode Command Line Tools, Rust, the .NET 10
# SDK), fetches the HLab submodules, builds the hook daemon and the UI, launches
# the app and opens the Accessibility settings the daemon needs.
# Later runs: pulls the latest commits of the current branch (when the checkout is
# clean), rebuilds what changed and relaunches.
#
# Build output goes to ~/Desktop/LittleBigMouse-build.log, so an error can be
# shared without scrolling the Terminal.

set -uo pipefail

cd "$(dirname "$0")" || exit 1
ROOT="$PWD"
BUILD_LOG="$HOME/Desktop/LittleBigMouse-build.log"
UI_LOG="${TMPDIR:-/tmp}/lbm-ui.log"
FIRST_RUN_MARK="$HOME/.local/share/LittleBigMouse/.mac-launcher-done"

B=$'\033[1m'; G=$'\033[32m'; Y=$'\033[33m'; R=$'\033[31m'; N=$'\033[0m'
step() { echo; echo "${B}==> $*${N}"; }
ok()   { echo "${G}    $*${N}"; }
warn() { echo "${Y}    $*${N}"; }

finish() {
    echo
    read -n 1 -s -r -p "Press any key to close this window."
    echo
    exit "${1:-0}"
}

fail() {
    echo
    echo "${R}${B}$*${N}"
    [[ -f "$BUILD_LOG" ]] && echo "Full log: $BUILD_LOG"
    finish 1
}

echo "${B}LittleBigMouse for macOS${N}  ($(git -C "$ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null || echo "no git"))"

# --- 1. Xcode Command Line Tools (compiler, linker, git) -------------------------
step "Checking Xcode Command Line Tools"
if xcode-select -p >/dev/null 2>&1; then
    ok "installed"
else
    xcode-select --install >/dev/null 2>&1
    fail "The Command Line Tools installer just opened. Finish it, then double-click this file again."
fi

# --- 2. Rust ---------------------------------------------------------------------
step "Checking Rust"
[[ -f "$HOME/.cargo/env" ]] && source "$HOME/.cargo/env"
if ! command -v cargo >/dev/null 2>&1; then
    warn "not found — installing rustup (official installer)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --profile minimal --default-toolchain stable \
        || fail "Rust installation failed."
    source "$HOME/.cargo/env"
fi
# rust-toolchain.toml pins the exact version; rustup fetches it on first use.
ok "$(cd "$ROOT/rust" && rustc --version 2>/dev/null || echo "rustup will install the pinned toolchain")"

# --- 3. .NET 10 SDK --------------------------------------------------------------
step "Checking the .NET SDK"
for d in /usr/local/share/dotnet /opt/homebrew/bin "$HOME/.dotnet"; do
    [[ -d "$d" && ":$PATH:" != *":$d:"* ]] && PATH="$d:$PATH"
done
has_net10() { command -v dotnet >/dev/null 2>&1 && dotnet --list-sdks 2>/dev/null | grep -Eq '^(1[0-9]|[2-9][0-9])\.'; }
if ! has_net10; then
    warn ".NET SDK 10+ not found — installing"
    if command -v brew >/dev/null 2>&1 || [[ -x /opt/homebrew/bin/brew ]]; then
        command -v brew >/dev/null 2>&1 || eval "$(/opt/homebrew/bin/brew shellenv)"
        warn "Homebrew may ask for your Mac password."
        brew install --cask dotnet-sdk || fail ".NET SDK installation failed."
    else
        # No Homebrew: Microsoft's per-user installer, no password needed.
        curl -sSfL https://dot.net/v1/dotnet-install.sh -o "${TMPDIR:-/tmp}/dotnet-install.sh" \
            && bash "${TMPDIR:-/tmp}/dotnet-install.sh" --channel 10.0 --install-dir "$HOME/.dotnet" \
            || fail ".NET SDK installation failed."
        PATH="$HOME/.dotnet:$PATH"
        export DOTNET_ROOT="$HOME/.dotnet"
    fi
    has_net10 || fail "The .NET SDK (10 or newer) still isn't visible. Open a new Terminal window and try again."
fi
ok "dotnet $(dotnet --version)"
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1

# --- 4. Source: latest commits and submodules ------------------------------------
step "Updating the source"
if [[ -d "$ROOT/.git" ]]; then
    if [[ -z "$(git -C "$ROOT" status --porcelain --untracked-files=no)" ]]; then
        if git -C "$ROOT" pull --ff-only --quiet 2>/dev/null; then
            ok "up to date with $(git -C "$ROOT" rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null || echo "remote")"
        else
            warn "couldn't pull (offline, or the branch diverged) — building what's here"
        fi
    else
        warn "you have local changes — not pulling, building what's here"
    fi
    git -C "$ROOT" submodule update --init --recursive --quiet \
        || fail "Couldn't fetch the HLab submodules (check your connection)."
    ok "submodules ready"
else
    [[ -f "$ROOT/HLab.Core/HLab.Core/HLab.Core.csproj" ]] \
        || fail "This folder isn't a git clone, so the HLab submodules are missing. Clone the repository with: git clone --recursive https://github.com/alextirgard00n/LittleBigMouse-Mac"
fi

# --- 5. Build ----------------------------------------------------------------------
: >"$BUILD_LOG"

step "Building the hook daemon (Rust) — the first build takes a few minutes"
(cd "$ROOT" && cargo build --release -p lbm-hook --manifest-path rust/Cargo.toml) 2>&1 \
    | tee -a "$BUILD_LOG" | grep -E '^(error|warning: unused)|Compiling lbm|Finished'
[[ ${PIPESTATUS[0]} -eq 0 && -x "$ROOT/rust/target/release/lbm-hook" ]] \
    || fail "The daemon build failed. The errors are in the log; copy them from there."
ok "lbm-hook built"

step "Building the UI (.NET)"
dotnet build -c Release "$ROOT/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia" 2>&1 \
    | tee -a "$BUILD_LOG" | grep -E ' error |Build succeeded|[0-9]+ Error'
[[ ${PIPESTATUS[0]} -eq 0 ]] || fail "The UI build failed. The errors are in the log; copy them from there."
ok "UI built"

# --- 6. Launch (run-lbm.sh stops a running copy, stages the daemon, relaunches) --
step "Launching"
"$ROOT/run-lbm.sh" --no-build >>"$BUILD_LOG" 2>&1 || fail "Launch failed."

sleep 4
if pgrep -f "LittleBigMouse.Ui.Avalonia" >/dev/null 2>&1; then
    ok "LittleBigMouse is running (menu bar / Dock). App log: $UI_LOG"
else
    echo "${R}The app exited right after starting. Last lines of its log:${N}"
    tail -n 40 "$UI_LOG" 2>/dev/null
    cp "$UI_LOG" "$HOME/Desktop/LittleBigMouse-app.log" 2>/dev/null \
        && echo "Copied to ~/Desktop/LittleBigMouse-app.log"
    finish 1
fi

# --- 7. Accessibility (first run only) -------------------------------------------
if [[ ! -f "$FIRST_RUN_MARK" ]]; then
    step "One more thing: Accessibility permission"
    echo "    The mouse hook needs Accessibility access. macOS gives it to the app"
    echo "    that launched LittleBigMouse, which is ${B}Terminal${N}."
    echo "    In the window that opens, switch on ${B}Terminal${N} (add it with + if"
    echo "    it isn't listed). Then click Start in LittleBigMouse; if it was already"
    echo "    started, it picks the permission up within a few seconds."
    open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    mkdir -p "$(dirname "$FIRST_RUN_MARK")" && touch "$FIRST_RUN_MARK"
fi

echo
echo "${G}${B}Done.${N} You can close this window; LittleBigMouse keeps running."
finish 0
