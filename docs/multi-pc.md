# Multi-PC mode (v6, Rust only)

One mouse and keyboard, on the **master**, drive the screens of other machines, the
**remotes**, the way Synergy/Deskflow/Barrier do. What LittleBigMouse adds is its
own model: the master holds a single physical layout, in millimetres, where its
monitors and the remotes' sit side by side, and its engine routes the pointer over
all of them. A remote only reports its screens and injects what it receives.

This document is the contract the lots below are written against. It states
decisions; where code exists, the code is the reference.

## Who does what

| Process | On the master | On a remote |
|---|---|---|
| `lbm-agent` | network node; pairing; merges the remote's screens into the layout; relays the input stream hook → network | network node; publishes its screens and their changes; relays the input stream network → hook |
| `lbm-hook` | captures mice (already) and, only while the pointer is on a remote, keyboards; runs the engine over every zone | injects pointer and keys (`SendInput` on Windows, uinput on Linux) |
| `lbm-app` | "Machines" panel; remote monitors on the map | accepts a pairing |

The network lives in the agent (`docs/v6-architecture-plan.md`, "ambition multi-PC").
The hook stays a local executor that never blocks on the network. Safety rule, both
ways: **when a hop of the input stream ends, the pointer is home** — the master's
hook brings its pointer back to a local zone and releases the keyboards; the
remote's hook releases every key and button the stream left pressed.

First supported pair: Linux master with the evdev backend → Windows remote. Out of
scope for now: clipboard, drag and drop, the Windows secure desktop (UAC, lock
screen), Windows as master, the portal and X11 backends as master.

## Network: crate `lbm-net`

- TCP, `TCP_NODELAY`, default port **24817**, TLS 1.3 with **mutual** self-signed
  certificates (`rustls` + `rcgen`). No CA: each side pins the other's certificate by
  its SHA-256 fingerprint, as the TV remotes do (`lbm-agent/src/remote/network.rs`).
- Identity: a key and certificate generated once, kept in the configuration
  directory (`peer-identity.pem`, 0600 / user-only ACL). The **machine id** is the
  fingerprint of that certificate.
- Every node with the mode enabled listens. The master connects to the address the
  user typed; roles are per link and are stated in `Hello`.
- Pairing is trust on first use, confirmed by the user **on both machines** against
  the displayed fingerprint. Known peers are stored in `peers.json`
  (`{ MachineId, Name, Address, Role }`). An unknown fingerprint gets `Hello` and
  nothing else until accepted.
- One connection carries two kinds of frame, each a four-byte little-endian length,
  one kind byte, and the body (the length counts the body only; bodies are bounded at
  1 MiB for control and 64 KiB for input, and anything else closes the link):
  - kind `0`, **control**: one JSON message (PascalCase, `Type` tag, unknown types
    tolerated);
  - kind `1`, **input**: one batch of `lbm_ipc::input` events, relayed unread.
- Control messages:
  - `Hello { Protocol, Name, Os, Version, Role: "Master" | "Remote", Capabilities }`
  - `Paired` / `Refused { Reason }` — the answer to a first `Hello`
  - `Displays { Revision, Displays: [PeerDisplay] }` — remote → master, at connection
    and on every display change
  - `SelectInput { Display, Value }` — either way: write VCP 0x60 on that display
    (see "Input switching")
  - `Ping` / `Pong` — every second while idle; three missed means the link is lost
  - `Bye { Reason }`
- `PeerDisplay` is OS-neutral and carries what the master's detection would have
  read itself: `{ Id, Connector, Name, Edid (base64 of the raw block, optional),
  X, Y, Width, Height (pixels of the remote's desktop), Scale, EffectiveDpi,
  Orientation, Frequency, Primary, Attached, WidthMm, HeightMm, InputSource
  (current VCP 0x60 value when it could be read) }`.

## Input stream: `lbm_ipc::input`

`InputEvent { Enter, Leave, Motion, Button, Wheel, Key }`, binary, absolute pointer
positions in the **receiver's** desktop pixels, Linux input codes for buttons and
keys (physical keys: the remote applies its own keyboard layout). See
`rust/crates/lbm-ipc/src/input.rs`.

Hook ↔ agent: the hook serves a second endpoint beside its command endpoint
(`input_endpoint_beside`), single client, raw batches both ways. The command
protocol only gains what configures it (`Load` already carries the zones).

On the master (lot 6, evdev backend only; the details are in
`lbm-hook/src/hook/linux/evdev/pipeline.rs`):

- The other machines' zones are **in play only while the input endpoint has a
  client**. An agent connects to it while it has a remote to relay to, and closes
  that connection when the remote link goes: the hook then brings the pointer home
  and the borders to the remote become walls again. Every other backend never puts
  them in play (`ZonesLayout::set_remote_enterable`).
- The hook never waits on the agent: a full queue closes the connection, which is
  the end of the hop. The agent ends the remote's stream in turn (its `Leave`), and
  may reconnect.
- A release goes where its press went: a button or key held across the border is
  released on the machine that saw it pressed. A keyboard is grabbed once none of
  its keys is held.
- The rescue shortcut is recognised in the grabbed key stream, with the same hold.
- One remote: batches carry no destination.

## Model

- **Cross-machine identity** of a panel: EDID manufacturer code + product code +
  serial (the serial string, else the binary serial). Never the checksum or the
  interface, which differ per input. Two displays from two machines with the same
  key are **one `Monitor` with two sources**.
- **The machine is carried by the source** (`DisplaySource.host: Option<String>`,
  the machine id; `None` is this machine), persisted as `Host` on `SourceDto`.
  `Monitor.sources` / `active_source` already exist.
- A monitor's `active_source` says which machine the panel is showing. Zones are
  compiled from active, attached sources only, so switching a shared panel is
  changing its active source.
- **Engine pixel space.** The engine keeps one integer pixel space. This machine's
  desktop is where it is; each remote's desktop is translated by a per-host offset
  chosen so that no two desktops overlap. A zone carries `Host` and the offset
  (`HostX`, `HostY` in the zones XML, absent for local zones); the hook subtracts
  the offset before emitting to that host.
- **Layout id**: the monitor set includes the remote monitors, so a connected
  remote selects its own profile; a remote appearing or leaving is a display change
  for the reconciler. Local-only helpers (`desktop_pixel_bounds`,
  `set_locations_from_system_configuration`, wallpaper, VCP targets, topology) see
  local sources only. A remote monitor never placed is put to the right of the
  local ones.
- The app detects displays by itself, so the agent exposes the remotes' through its
  API (`Peers`), announced by the `MultiPc` capability.

## Agent (lot 5a): `lbm-agent/src/peers`

- **Settings**: `multi-pc.json` in the configuration directory,
  `{ Enabled (false), Port (24817), Name (absent: the host name) }`, written atomically.
  Off, the mode costs nothing: no listener, no task, no timer, and no identity is made
  before it is first turned on.
- **Roles on the wire**: the listener says `Remote` in its `Hello`, a node that dials
  says `Master`; a link where both say the same is refused. The stored `Role` of a peer
  is the one it stated.
- **Pairing**: a fingerprint in `peers.json` is accepted at once; any other waits as
  `Pending` (holding its connection, for as long as the link's pairing timeout) until
  the user accepts (stored, then `Paired` is said) or refuses. A paired remote whose
  address is known is called back while the mode is on: after 1 s, doubling, at most
  30 s. A different fingerprint at that address is refused, not paired.
- **Remote side**: `Displays` at link-up, then after every layout rebuild of the agent
  (its display-change path; no watcher of its own), under a new `Revision` only when
  the screens differ. `PeerDisplay` comes from the platform detection (`LinuxMonitor`
  in logical pixels, the Win32 tree), raw EDID from sysfs or the registry; `InputSource`
  is not read yet (lot 8). Input batches go to the hook's input endpoint, connected at
  the first batch and again after a failure; with no connection they are dropped. The
  end of the link closes that connection: the hook releases what is held.
- **Master side**: the latest `Displays` per remote (highest revision) are kept and
  shown through the API; the hook's batches go to the linked remote with `try_input`.
  **One remote at a time**: the most recently linked one gets the stream until lot 6
  routes by zone. `SelectInput` is logged and ignored (lot 8).
- **API** (additive to protocol 3, capability `MultiPc`):
  - `Peers` → `{ Enabled, Port, Name, MachineId (this machine's fingerprint, null before
    the first use), ListenAddress, Error, Peers: [{ MachineId, Name, Address, Connected,
    Displays, Role, State: "Paired" | "Pending" }] }`;
  - `SetMultiPc { Enabled, Port?, Name? }` → the same state, once the listener is up or
    everything is closed;
  - `PeerConnect { Address }` (`host`, `host:port`, `[v6]:port`) → that machine's entry
    once it said `Hello`;
  - `PeerAccept`, `PeerRefuse`, `PeerForget` `{ MachineId }` → `null`;
  - event `{ "Event": "Peers", "Peers": <state> }` to subscribers on every change.
- `cargo run -p lbm-agent --example peer_drive -- ADDR ID_DIR [script]` is a master
  without a hook, for trying a remote by hand (see the example's header).

## The master's layout (lot 5b)

- **Which screens**: those of every machine in `peers.json` linked now as a **remote** of
  this one (it said `Remote` in its `Hello`) that has sent its `Displays`, in machine-id
  order. A machine this one is the remote of is never merged, nor one pending.
- **A display change**: such a machine linking, leaving, or sending other screens goes
  through the reconciler exactly as a local display change (`Input::DisplayChanged`:
  debounce, settle loop, rebuild, profile, `Load`+`Run` if the layout is enabled). The
  display signature carries a hash of the remotes' screens, since a remote sharing only
  panels this machine has leaves the layout id unchanged.
- **Building it**, in this order — the agent's rebuild, and what a frontend must do to
  hold the same layout (same id, same zones):
  1. this machine's displays mapped (`lbm_layout::linux::populate` /
     `windows::populate`, which compute an id and then call the load closure);
  2. in that closure, before anything else, `lbm_display::peer::merge_peer(layout,
     machine_id, name, &displays)` for each remote, in machine-id order, with the
     `Displays` of the `Peers` state (`Name` is the peer's);
  3. `layout.id = layout.compute_id()` (only when something was merged);
  4. the store loaded (`LayoutPersistence::load`); `populate` then places what the store
     did not (remote monitors to the right of the local ones) and anchors.
- A new set of monitors is a new profile, **off** until the user enables it, as for any
  unknown layout: the first time a remote with screens of its own links, the engine
  stops on it.
- **Local only**: `Desktop` in `Load` is this machine's desktop — a shared panel showing
  the other machine is still one of its outputs, so its local source counts; wallpaper,
  VCP targets, topology (`ApplyTopology`, `ChangeOutput`), the gap guard and the screens
  reported to a master see local sources only.
- **Documents**: an edit, preview, probe or topology document is refused when the
  machines its sources name (`Host`) are not those of the current layout, even under the
  same `LayoutId`: an edit made with a remote linked is not applied to the layout made
  after it left, nor the other way round.
- **`SetActiveSource { LayoutId, Monitor, Source }`** (API, `MultiPc`): shows `Source` (one
  of the monitor's sources, attached to its machine's desktop) on the panel, saves, and
  hands a running engine the new zones; answered `null`. The VCP 0x60 write of input
  switching (lot 8) plugs in where the runtime handles it.

## A panel both machines know

- If the panel is attached to one machine only, that source is active.
- If both report it attached (a KVM that keeps both links alive, or two inputs of
  one monitor), the last choice stands; the user changes it from the map, the tray
  or a shortcut.

### Input switching (no KVM)

For a monitor wired to both machines on two inputs, choosing the active source
also **switches the monitor**: VCP 0x60 is written with the input value of the
chosen source. Each source remembers its input value (`InputSource`, learnt by
reading 0x60 on the machine currently shown, on display events — never polled).
The write is done by the master when it can reach the monitor over DDC, else it is
asked of the remote with `SelectInput`. With a hardware KVM on a single input
there is no value to write and only the active source changes.

## Lots

1. Contracts: this document, `lbm_ipc::input`.
2. `lbm-net`: identity, TLS, pairing store, frames, control messages.
3. Model: host on sources, cross-machine key, merge, layout id, zones with host.
4. Remote injection in `lbm-hook`: input endpoint, `SendInput`, uinput.
5. Agent: peer manager, API, input relay, remote displays in the world.
6. Master hook (evdev): remote sink, keyboard grab, coming home.
7. egui: Machines panel, remote monitors, source switch.
8. Input switching over VCP 0x60 (Windows DDC included).
