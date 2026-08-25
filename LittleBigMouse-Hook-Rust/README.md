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

## Benchmarks

```
cargo bench --bench mouse_engine -- --quick   # timings, short mode (~1 min)
cargo bench --bench mouse_engine              # timings, full statistics
cargo bench --bench alloc_profile             # allocation counts
cargo bench --bench mouse_engine -- --test    # run every scenario once, no timing
```

`benches/mouse_engine.rs` times the traversal core, `benches/alloc_profile.rs`
counts its allocations, and `benches/support/` holds the fixtures both share.
Nothing installs a hook, opens a device or moves the real pointer: the engine
talks to a `CursorEnv` whose every method is a field read, so what is measured is
the algorithm and not the OS. Layouts are generated as the XML the C# UI would
send (2, 6 and 16 monitors, mixed DPI; plus a 4x4 wall), which is why there is no
16-monitor fixture file in the tree.

Each scenario is a closed loop of mouse events, and `Scenario::verify` replays it
eight times asserting that every event still takes the branch the benchmark is
named after — a crossing benchmark that quietly decayed into interior moves would
fail rather than report a great number.

### Reading the results

**Timings are machine-dependent.** They move with the CPU, the clock governor,
the allocator and the build flags, so only compare runs made on the same machine,
ideally back to back — `cargo bench` keeps a baseline per benchmark id under
`target/criterion/` and prints the change automatically. The figures below are a
reference point, not a threshold; nothing in the suite fails on a timing.

Measured on an AMD Ryzen 9 9950X, Linux 7.2 (CachyOS), rustc 1.94.0, `--quick`:

| scenario | 2 zones | 6 zones | 16 zones |
|---|---|---|---|
| `interior` (Strait / Cross) | 11.7 / 11.5 ns | — | — |
| `crossing/Strait` | 66 ns | 68 ns | 67 ns |
| `crossing/Cross` | 91 ns | 253 ns | 660 ns |
| `no_target/Strait` | 7.0 ns | 7.0 ns | 7.1 ns |
| `no_target/Cross` | 27 ns | 107 ns | 296 ns |
| `intersection/cross_4x4_diagonal` | | | 545 ns |
| `travel_pixels/cached` | 15 ns | 15 ns | 15 ns |
| `travel_pixels/compute` | | row_16 11 ns | grid_4x4 2.07 µs |
| `load_layout` | 10.4 µs | 34.2 µs | 93.3 µs |

**Allocation counts are not machine-dependent** — they are exact and identical on
any host for a given build, which makes them the half of the baseline worth
diffing:

| scenario | allocations |
|---|---|
| `interior`, both algorithms | **0 per event** |
| `crossing/Strait`, any zone count | 1 per event (the cached travel-path `Vec` clone) |
| `crossing/Cross` | 1 / 5 / 15 per event at 2 / 6 / 16 zones |
| `no_target/Cross` | 1 / 5 / 15 per event at 2 / 6 / 16 zones |
| `travel_pixels/compute/grid_4x4` | 460 per call |
| `load_layout/16` | 552 per call, 357 kB |

Two shapes fall out of this, both expected from the code rather than surprises:
Strait resolves one link on one side and is flat in the zone count, while Cross
scans every zone per event and costs one `Rect::intersect` `Vec` for each.
Neither is currently a problem — 660 ns leaves a wide margin at a 1 kHz report
rate — and this task deliberately changed no algorithm; the point is to have the
numbers before anyone does.

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
