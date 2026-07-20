# LittleBigMouse.Hook — Rust port

A memory-safe Rust rewrite of the native C++ `LittleBigMouse.Hook` daemon: the
separate process that installs the low-level Windows mouse hook and repositions
the cursor across multi-DPI monitors.

The daemon is language-agnostic behind its contract with the C# UI: authenticated
local IPC exchanges length-prefixed UTF-8 XML, so this process is a drop-in
replacement for the C++ one. Windows uses a named pipe restricted to the current
user and logon session; Linux uses a Unix-domain socket with mode `0600`.

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
| `ipc/` | `Remote/` — bounded local IPC, length framing, `CommandMessage`/`DaemonMessage` |
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
`stage.ps1` and CI both do this when staging the portable application.

## Environment overrides

| Variable | Effect |
|---|---|
| `LBM_HOOK_UI` | Force UI mode (wait for IPC commands) instead of parent-process detection — used by test scripts |
| `LBM_HOOK_DEBUG` | Print a stderr heartbeat: `hooked` / `mouse_events` / `crossings` |

## Shipped implementation

The memory-safe Rust daemon is the only hook built, staged, and packaged. The
legacy C++ source remains as historical porting reference but is not part of the
solution or any distributable path.
