#!/usr/bin/env bash
#
# run-lbm.sh — Dev launcher for the in-progress LittleBigMouse build on Linux.
#
# Linux counterpart of run-lbm.ps1. Stops any running instance, builds the Rust
# hook daemon + the Avalonia UI, stages the daemon next to the UI (lbm-hook), and
# relaunches the app. No elevation is needed on Linux — but the evdev/uinput hook
# backend needs read access to /dev/input and write access to /dev/uinput (be in
# the `input` group, or install a udev rule); without it the daemon falls back to
# the Wayland portal / X11 backend.
#
# Usage (from anywhere):
#   ./run-lbm.sh [options]
#
# Options:
#   -c, --config <Release|Debug>   build configuration (default: Release)
#       --no-build                 skip compilation, just (re)stage and relaunch
#       --skip-daemon              don't build/stage the Rust daemon (UI only)
#       --no-launch                build/stage only, don't launch the app
#   -h, --help                     show this help

set -uo pipefail

CONFIG="Release"
NO_BUILD=0
SKIP_DAEMON=0
NO_LAUNCH=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        -c|--config)   CONFIG="${2:-}"; shift 2 ;;
        --no-build)    NO_BUILD=1; shift ;;
        --skip-daemon) SKIP_DAEMON=1; shift ;;
        --no-launch)   NO_LAUNCH=1; shift ;;
        -h|--help)     grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 2 ;;
    esac
done

if [[ "$CONFIG" != "Release" && "$CONFIG" != "Debug" ]]; then
    echo "Invalid --config '$CONFIG' (expected Release or Debug)" >&2
    exit 2
fi

# --- paths --------------------------------------------------------------------
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UI_PROJ="$ROOT/LittleBigMouse.Ui/LittleBigMouse.Ui.Avalonia"
BIN_DIR="$UI_PROJ/bin/$CONFIG/net10.0"
UI_DLL="$BIN_DIR/LittleBigMouse.Ui.Avalonia.dll"
UI_APPHOST="$BIN_DIR/LittleBigMouse.Ui.Avalonia"
RUST_DIR="$ROOT/LittleBigMouse-Hook-Rust"
# Release -> target/release, Debug -> target/debug (matches the UI's config, which
# is what LittleBigMouseClientService.FindHookPath prefers).
if [[ "$CONFIG" == "Release" ]]; then RUST_PROFILE_DIR="release"; else RUST_PROFILE_DIR="debug"; fi
RUST_BIN="$RUST_DIR/target/$RUST_PROFILE_DIR/lbm-hook"
STAGED_HOOK="$BIN_DIR/lbm-hook"
LOG="${TMPDIR:-/tmp}/lbm-ui.log"

# Dev version stamp: v5.4.1 + 12 commits -> 5.4.1.12, so the About window tells
# dev builds apart from releases. Empty (Directory.Build.props version applies)
# when git or the tags are unavailable.
VERSION="$(git -C "$ROOT" describe --tags --long --match 'v*' 2>/dev/null \
    | sed -E 's/^v//; s/-([0-9]+)-g[0-9a-f]+$/.\1/')"

# Colours (skipped when not a TTY).
if [[ -t 1 ]]; then C='\033[36m'; D='\033[90m'; Y='\033[33m'; G='\033[32m'; R='\033[31m'; N='\033[0m'
else C=''; D=''; Y=''; G=''; R=''; N=''; fi
step() { echo -e "\n${C}==> $*${N}"; }
note() { echo -e "    ${D}$*${N}"; }
warn() { echo -e "${Y}!!  $*${N}"; }

# 1. stop running instance(s). The bracketed letter keeps pkill from matching its
#    own command line. Three patterns: `dotnet …dll` launches, apphost launches
#    (the script prefers the apphost), and a distribution package launched through
#    its /usr/bin/littlebigmouse entry point — whose command line carries none of
#    the assembly name, so the first two patterns walk straight past it. Miss any
#    of them and the old instance survives, the relaunch dies on the
#    single-instance lock, and you are left looking at the stale app with an empty
#    log. EVIOCGRAB releases on process exit, so this also frees any grabbed mice.
#
#    ORDER AND WAITING BOTH MATTER, and for the same reason: the UI relaunches the
#    daemon whenever it loses the connection. That contract is only suspended for a
#    clean tray Exit (`_quitting`, see "Stop relaunching the daemon the UI is
#    quitting"); a signal does not go through QuitAsync, so a SIGTERM'd UI keeps its
#    reconnect loop running while it winds down. Kill the daemon while any UI is
#    still breathing and it spawns a REPLACEMENT, which survives into the launch
#    below still holding the mice — and with a different PID, so it reads as a kill
#    that failed rather than a spawn that should not have happened.
#
#    pkill only delivers the signal, it does not wait. So: signal the UI, wait for
#    it to actually be gone, and only then take the daemon.
step "Stopping running instance(s)"

# Anchored patterns: /usr/lib/littlebigmouse/lbm-hook must survive these and be left
# to the daemon pkill below. The bracketed letter keeps pkill off its own command line.
UI_PATTERNS=(
    "LittleBigMouse.Ui.Avalonia.dl[l]"
    "LittleBigMouse.Ui.Avaloni[a]$"
    "littlebigmous[e]$"
)

ui_running() {
    local pattern
    for pattern in "${UI_PATTERNS[@]}"; do
        pgrep -f "$pattern" >/dev/null 2>&1 && return 0
    done
    return 1
}

wait_gone() {  # wait_gone <deciseconds> <predicate>
    local tries="$1" predicate="$2"
    while (( tries-- > 0 )); do
        "$predicate" || return 0
        sleep 0.1
    done
    return 1
}

if ui_running; then
    for pattern in "${UI_PATTERNS[@]}"; do pkill -f "$pattern" 2>/dev/null; done
    if wait_gone 50 ui_running; then
        note "UI stopped"
    else
        warn "UI still up 5s after SIGTERM — forcing"
        for pattern in "${UI_PATTERNS[@]}"; do pkill -9 -f "$pattern" 2>/dev/null; done
        wait_gone 20 ui_running || warn "UI survived SIGKILL — it may respawn the daemon below"
    fi
else
    note "no UI running"
fi

# Nothing is left to respawn it now.
hook_running() { pgrep -x lbm-hook >/dev/null 2>&1; }
if hook_running; then
    pkill -x lbm-hook 2>/dev/null
    wait_gone 30 hook_running && note "daemon stopped" || warn "daemon did not exit"
else
    note "no daemon running"
fi

# A daemon back from the dead means something outside this script relaunched it.
# Say so loudly: it is holding the mice, and building on top of it is how you end up
# debugging the previous build's grab.
sleep 0.5
if hook_running; then
    warn "a daemon reappeared after the kill (pid $(pgrep -x lbm-hook | tr '\n' ' ')) — it holds the mice"
fi

# 2. build the Rust daemon (non-fatal: stage whatever already exists on failure).
if [[ "$NO_BUILD" -eq 0 && "$SKIP_DAEMON" -eq 0 ]]; then
    step "Building Rust hook daemon  (cargo build, $CONFIG)"
    if command -v cargo >/dev/null 2>&1; then
        CARGO_ARGS=(build --manifest-path "$RUST_DIR/Cargo.toml")
        [[ "$CONFIG" == "Release" ]] && CARGO_ARGS+=(--release)
        cargo "${CARGO_ARGS[@]}" || warn "cargo build failed — staging existing binary"
    else
        warn "cargo not available — staging existing binary"
    fi
fi

# 3. build the UI
if [[ "$NO_BUILD" -eq 0 ]]; then
    step "Building UI  ($CONFIG${VERSION:+, v$VERSION})"
    BUILD_ARGS=("$UI_PROJ" -c "$CONFIG" --nologo -v m)
    [[ -n "$VERSION" ]] && BUILD_ARGS+=("-p:Version=$VERSION")
    dotnet build "${BUILD_ARGS[@]}" || { echo -e "${R}dotnet build failed${N}"; exit 1; }
fi

# 4. stage the daemon next to the UI (FindHookPath checks the sibling first).
if [[ "$SKIP_DAEMON" -eq 0 ]]; then
    if [[ -f "$RUST_BIN" ]]; then
        step "Staging daemon  ->  lbm-hook"
        cp -f "$RUST_BIN" "$STAGED_HOOK"
        note "$STAGED_HOOK"
    else
        warn "Rust daemon not found: $RUST_BIN (skipping staging)"
    fi
    # lbm-pattern: native-Wayland test pattern viewer (VCP plugin helper).
    PATTERN_BIN="$RUST_DIR/target/$RUST_PROFILE_DIR/lbm-pattern"
    if [[ -f "$PATTERN_BIN" ]]; then
        cp -f "$PATTERN_BIN" "$BIN_DIR/lbm-pattern"
        note "$BIN_DIR/lbm-pattern"
    fi
fi

# 5. launch (detached; logs to $LOG so the terminal stays free)
if [[ "$NO_LAUNCH" -eq 0 ]]; then
    if [[ -x "$UI_APPHOST" ]]; then
        LAUNCH=("$UI_APPHOST")
    elif [[ -f "$UI_DLL" ]]; then
        LAUNCH=(dotnet "$UI_DLL")
    else
        echo -e "${R}UI not found: $UI_APPHOST or $UI_DLL${N}"; exit 1
    fi
    step "Launching LittleBigMouse"
    nohup "${LAUNCH[@]}" >"$LOG" 2>&1 &
    disown
    note "pid $! — logs: $LOG"
fi

echo -e "\n${G}Done.${N}"
