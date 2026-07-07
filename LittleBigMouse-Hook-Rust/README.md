# LittleBigMouse.Hook — Rust port

A memory-safe Rust rewrite of the native C++ `LittleBigMouse.Hook` daemon: the
separate process that installs the low-level Windows mouse hook and repositions
the cursor across multi-DPI monitors.

The daemon is language-agnostic behind its contract with the C# UI — a loopback
TCP socket (port **25196**) exchanging `\n`-delimited XML — so this process is a
drop-in replacement for the C++ one.

## Why Rust

The C++ daemon manages the zone graph with raw owning pointers
(`Zone*`/`ZoneLink*`/`unordered_map<Zone*>`), the source of recurring
use-after-free crashes (a stale `_oldZone` after a hot layout reload, async client
deletion, etc.). Here the zone graph lives in a generational [`slotmap`] arena
keyed by `ZoneId`: a reload drops the arena and bumps generations, so any stale
id resolves to `None` instead of dereferencing freed memory — the crash class is
unrepresentable. The only `unsafe` is the thin Win32 FFI layer; the engine,
zones, geometry and IPC are 100% safe.

## Layout

| Module | Ports the C++ |
|---|---|
| `ipc/` | `Remote/` — TCP server, `\n` framing, `CommandMessage`/`DaemonMessage` |
| `hook/` | `Hook/Hooker*` — `WH_MOUSE_LL`, WinEvents, display window, message pump |
| `geometry/` | `Geometry/*.h` — `Point`/`Rect`/`Line`/`Segment` over a `Coord` trait |
| `zones/` | `Engine/Zone`,`ZoneLink`,`ZonesLayout` on the arena |
| `engine/` | `Engine/MouseEngine` — Strait/Cross traversal, resistance, freelook |
| `platform/` | `MouseHelper` + process/parent detection |

The engine talks to the OS only through the `CursorEnv` trait, so the whole
traversal algorithm runs deterministically under a fake cursor in tests.

## Build & test

```
cargo build
cargo test
cargo clippy --all-targets
```

## The exe name

Cargo rejects a target named with a `.`, so the binary builds as **`lbm-hook.exe`**
and must be renamed to **`LittleBigMouse.Hook.exe`** (the name the UI's
`FindHookPath` / `GetProcessesByName` and the installer expect) when staging.
`stage.ps1` does this; CI does it in the "Stage hook" step when `HOOK_IMPL=rust`.

## Environment overrides

| Variable | Effect |
|---|---|
| `LBM_HOOK_PORT` | Listen on a non-default port (side-by-side testing next to a running C++ daemon; the port is irrelevant to the global hook itself) |
| `LBM_HOOK_UI` | Force UI mode (wait for socket commands) instead of parent-process detection — used by test scripts |
| `LBM_HOOK_DEBUG` | Print a stderr heartbeat: `hooked` / `mouse_events` / `crossings` |

## CI coexistence

`.github/workflows/dotnet-desktop.yml` builds the C++ hook by default
(`HOOK_IMPL=cpp`). A manual `workflow_dispatch` with `hook_impl=rust` builds and
ships this daemon instead. The C++ project stays in the solution until the Rust
port reaches full parity.
