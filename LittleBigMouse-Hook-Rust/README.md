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
cargo bench --bench evdev_pump                # Linux pump: allocations + timings
cargo bench --bench linux_pipeline            # Linux end-to-end: engine + plumbing
cargo bench --bench linux_pipeline -- --quick # same, short mode (fewer iterations)
```

`benches/mouse_engine.rs` times the traversal core, `benches/alloc_profile.rs`
counts its allocations, `benches/evdev_pump.rs` measures the Linux input
plumbing around them, `benches/linux_pipeline.rs` drives the two together as one
end-to-end pipeline, and `benches/support/` holds the fixtures they share.
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

### The evdev pump (Linux)

`benches/evdev_pump.rs` measures what surrounds the engine on Linux: one pump
cycle — rebuild the `poll` set, drain a device's events, route each of them to
the pointer or the keyboard batch, compose the uinput frames. No device, no
`/dev/uinput`, no grab; it runs unprivileged and never touches the real mice. Off
Linux it prints a line and exits.

The two modes are the same routing code over the same events, differing only in
where the buffers live: **owned**, the current pump, whose buffers belong to the
`Router` and are reused cycle after cycle, and **per-cycle**, a frozen copy of
the pump body from before that (poll set, `fetch_events` drain, pending batches
and emitted frame all allocated as locals, every cycle).

| scenario | allocs/cycle (owned → per-cycle) | ns/cycle (owned → per-cycle) |
|---|---|---|
| `motion` — the 1 kHz report | **0** → 3 (144 B) | 8.4 → 35.8 ns |
| `motion+wheel` | **0** → 5 (336 B) | 13.6 → 68.3 ns |
| `button` | **0** → 5 (264 B) | 9.3 → 61.9 ns |
| `keyboard` — a macro key on a combined node | **0** → 6 (384 B) | 11.4 → 77.0 ns |
| `combined` — mouse and keyboard in one report | **0** → 6 (456 B) | 13.7 → 76.4 ns |
| `partial` — one report split across two reads | **0** → 3 (108 B) | 6.3 → 34.6 ns |
| `burst/8` — eight reports in one drain | **0** → 10 (984 B) | 38.1 → 123.2 ns |

Same machine and toolchain as the table above. The allocation columns are exact;
the timings are indicative, and slightly pessimistic for the per-cycle mode,
which pays the counting allocator's two relaxed atomics on every allocation it
makes. The engine's own cost is *not* in these numbers — add the `interior` or
`crossing` line above for a whole frame.

What matters here is the zero, not the nanoseconds: at 1 kHz per device the pump
used to call into the global allocator three to ten times per report, on the
thread that owns the user's grabbed mice, where a slow path through the allocator
is a visible stall. `a_steady_stream_stops_allocating` (in `hook/linux/evdev.rs`)
guards it from the test suite, by asserting no buffer grows again once the first
frames have sized it.

### The whole pipeline (Linux)

`benches/evdev_pump.rs` isolates the plumbing and `benches/mouse_engine.rs`
isolates the engine; `benches/linux_pipeline.rs` puts them together and drives the
combination the way a live session does — a stream of mouse reports at a fixed
polling rate, each one accumulated into a frame, run through
`MouseEngine::on_mouse_move`, and composed into the pointer/keyboard frames the two
virtual devices would be handed. It reproduces the body of `Router::flush_frame`
(acceleration curve, sub-pixel remainder, engine call, clamp, frame composition)
minus the two things that need privilege or hardware: the `uinput` writes and the
`Shared` lock. No device, no `/dev/uinput`, no grab, no root; off Linux it prints a
line and exits.

Four report streams, each a closed loop over the shared synthetic layouts:
**interior** (moves that stay on one monitor), **crossing** (moves that step over a
border and back, so the engine repositions the cursor every report), **partial**
(every report split across two reads — the pump flushes a partial frame and
completes it next cycle), and **combined** (motion with a keyboard usage riding the
same reports, so both virtual devices get a frame). `Stream::verify` replays each
loop eight times asserting it still crosses (or still does not), so a stream that
decayed into the wrong branch fails rather than reporting a meaningless number.

Each stream is swept over the 125 / 500 / 1000 Hz report cadences of a low-end, a
mid and a 1 kHz gaming mouse. The work per report does not depend on the rate — the
rate only sets the per-report budget (8 ms at 125 Hz, 1 ms at 1 kHz) — so the
latency and allocation columns repeat across rates and the **budget %** column is
what moves: it is the per-report time as a fraction of that budget, i.e. the
processing-depth / lag figure. Below 100 % the pump keeps up with that mouse on one
core; the rest is headroom for a burst or a second grabbed device.

Measured on the same AMD Ryzen 9 9950X / Linux 7.2 (CachyOS) / rustc 1.94.0 as the
tables above, `--quick`. The rate is omitted below because the ns/frame and
allocation figures are identical across the three; only the budget % differs, and
it never exceeds ~0.02 % even at 1 kHz:

| stream | 2 zones | 6 zones |
|---|---|---|
| `interior` | 39 ns, **0 allocs** | 39 ns, **0 allocs** |
| `crossing` | 105 ns, 1 alloc (32 B) | 220 ns, 5 allocs (288 B) |
| `partial` | 20 ns/cycle, **0 allocs** | 20 ns/cycle, **0 allocs** |
| `combined` | 40 ns, **0 allocs** | 40 ns, **0 allocs** |

The allocation columns are exact and match the engine's own profile: the plumbing
adds none, and the only per-frame allocation is the cached travel-path `Vec` clone
the Cross engine already pays on a crossing (1 / 5 per event at 2 / 6 zones). The
`partial` figure is per pump cycle, and a split report is two cycles (the second is
just the `SYN_REPORT`), so it reads lower than a whole report — the sum of the two
cycles is the comparable number. Even the worst line, a 6-zone crossing at 220 ns,
sits at 0.02 % of the 1 ms budget: the pipeline has a wide margin at a 1 kHz report
rate, which is the point of having the baseline before anyone touches the
algorithm. This task changed none of it.

To take a real end-to-end measurement later — against actual devices and a real
`uinput` node rather than synthetic reports — the harness deliberately stops short
of two privileged steps: grabbing `/dev/input/event*` (needs root or an `input`
group membership) and creating a `/dev/uinput` device (same). A privileged variant
would grab one real mouse, build the two virtual devices as `Router::arm` does, and
time from `poll` return to `virt.emit`; `examples/enum_bench.rs` and the arm path in
`hook/linux/evdev/router.rs` are the templates. That is out of scope here, and CI
runs unprivileged, so the synthetic streams are what the recorded baseline uses.

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
