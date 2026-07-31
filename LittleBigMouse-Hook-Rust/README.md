# LittleBigMouse.Hook — the daemon

The separate process that installs the low-level mouse hook and repositions the
cursor across multi-DPI monitors. A memory-safe Rust rewrite of the native C++
daemon it replaced, and since 5.4 the only one — the C++ implementation was
removed from the tree once it had stopped speaking the current wire protocol.

The daemon is language-agnostic behind its contract with the C# UI — per-user
local IPC (a per-session named pipe on Windows, a 0600 Unix socket in
`$XDG_RUNTIME_DIR` on Linux) exchanging length-prefixed UTF-8 XML frames.

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
| `ipc/` | `Remote/` — local IPC server, u32-length framing, `CommandMessage`/`DaemonMessage` |
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
`stage.ps1` does this; CI does it in the "Stage hook next to UI output" step.

## Environment overrides

| Variable | Effect |
|---|---|
| `LBM_HOOK_ENDPOINT` | Listen on a non-default endpoint — the **full** pipe path on Windows (`\\.\pipe\my-test`), the socket path elsewhere. The UI honours it too, so a test daemon/UI pair runs side by side with the production pair |
| `LBM_HOOK_UI` | Force UI mode (wait for socket commands) instead of parent-process detection — used by test scripts |
| `LBM_HOOK_DEBUG` | Print a stderr heartbeat: `hooked` / `mouse_events` / `crossings` |

## CI

`.github/workflows/dotnet-desktop.yml` builds and ships this daemon, and runs its
tests alongside the managed ones. It is the only hook: the C++ implementation was
removed once this port had passed it, having stopped speaking the current wire
protocol.
