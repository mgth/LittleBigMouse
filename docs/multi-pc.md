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

First supported pair: Linux master with the evdev backend → Windows remote. Windows
as master came next (its low-level hooks, below). Out of scope for now: drag and drop,
the Windows secure desktop (UAC, lock screen), the portal and X11 backends as master.

## Network: crate `lbm-net`

- TCP, `TCP_NODELAY`, default port **24817**, TLS 1.3 with **mutual** self-signed
  certificates (`rustls` + `rcgen`). No CA: each side pins the other's certificate by
  its SHA-256 fingerprint, as the TV remotes do (`lbm-agent/src/remote/network.rs`).
- Identity: a key and certificate generated once, kept in the configuration
  directory (`peer-identity.pem`, 0600 / user-only ACL). The **machine id** is the
  fingerprint of that certificate.
- Every node with the mode enabled listens. The master connects to the address the
  user typed; roles are per link and are stated in `Hello`.
- Discovery (`lbm-net/src/discovery.rs`): with the mode on, a node also keeps a UDP
  socket on **24817**. It broadcasts a `Query` on each of its IPv4 networks when the
  mode starts (again after 1 s and 3 s), answers every `Query` heard with an `Announce`
  to the sender, and says `Bye` when the mode stops; no other datagram, no timer. A
  datagram is `{ Lbm: "LittleBigMouse", V: 1, Kind, MachineId, Name, Port }`, at most
  1 KiB. What is heard is only proposed (`Discovered` in the state); pairing is still
  `PeerConnect` and the fingerprint check on both machines. A paired remote heard at a
  new IP address is dialled there as its fingerprint, and the new address is stored only
  once that link is up. Choice and limits: `multi-pc-etat.md`, "Découverte
  automatique".
- Pairing is trust on first use, confirmed by the user **on both machines** against
  the displayed fingerprint. Known peers are stored in `peers.json`
  (`{ MachineId, Name, Address, Role }`). An unknown fingerprint gets `Hello` and
  nothing else until accepted.
- One connection carries two kinds of frame, each a four-byte little-endian length,
  one kind byte, and the body (the length counts the body only; bodies are bounded at
  1 MiB for control and 64 KiB for input, and anything else closes the link):
  - kind `0`, **control**: one JSON message (PascalCase, `Type` tag, unknown types
    tolerated);
  - kind `1`, **input**: one batch of `lbm_ipc::input` events, relayed unread;
  - kind `2`, **bulk**: one piece of a transfer — the transfer's number (8 bytes, little
    endian), a flags byte (bit 0: last piece), then at most 16 KiB of data. Only sent to
    a peer that announced the `Clipboard` capability: an older one would read an unknown
    kind as a broken link. See "Shared clipboard" for why it never delays input.
- Control messages:
  - `Hello { Protocol, Name, Os, Version, Role: "Master" | "Remote", Capabilities }`
  - `Paired` / `Refused { Reason }` — the answer to a first `Hello`
  - `Displays { Revision, Displays: [PeerDisplay] }` — remote → master, at connection
    and on every display change
  - `Wallpapers { Revision, Wallpapers: [{ Display, Image | Color }] }` — remote →
    master, at link-up, after every display change and when the remote's desktop says
    its wallpaper changed (see "Wallpapers"); added after
    protocol 1 without a new version, an older master ignores it as an unknown type
  - `SelectInput { Display, Value, Request? }` — either way: write VCP 0x60 on that
    display (see "Input switching"); `Request` numbers it for the answer, a retry of the
    same value being otherwise indistinguishable from the attempt before it (added
    after protocol 1, absent from an older master)
  - `InputReset` — remote → master: the remote ended the session of the input stream
    while the link lives on (its hook could not keep up, its connection to the hook
    ended — the hook restarted, crashed, or failed to read in time — or a crossing found
    no hook to take it); the master brings its pointer home (see "The end of a hop"). Added after protocol 1 without a new version, an older
    master ignores it as an unknown type
  - `InputSwitched { Display, Value, Request?, Error? }` — remote → master: how the
    `SelectInput` numbered `Request` went, `Error` saying why it failed; the master
    numbers each choice it accepts for a panel at once, and heeds only what is said of
    its latest choice for that panel (see "Input
    switching"). Added after protocol 1 without a new version, an older master ignores
    it as an unknown type and an older remote sends none
  - `ClipboardOffer { Serial, Formats }`, `ClipboardRequest { Serial, Format, Transfer }`,
    `ClipboardAbort { Transfer, Reason }` — either way, between peers that both announced
    `Clipboard`: see "Shared clipboard"
  - `Overlay { Revision, Bands: [Band] }` — master → remote: the rulers to draw on the
    remote's screens, in place of any drawn before; empty takes them down (see "Rulers
    on a remote's screens"). Sent only to a remote whose `Hello` lists the capability
    `RemoteOverlay`; added after protocol 1 without a new version
  - `OverlayFailed { Revision, Error }` — remote → master: the rulers of `Revision`
    are not shown, refused or taken down, and why
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

On the master (lot 6; the rules are in `lbm-hook/src/hook/crossing/`,
platform-neutral; what evdev adds to them is in
`lbm-hook/src/hook/linux/evdev/pipeline.rs`, what the Windows low-level hooks add in
`lbm-hook/src/hook/ll_master.rs` and `hook/windows/master.rs`, see "The Windows master"
below):

- The other machines' zones are **in play only while the input endpoint has a
  client**. An agent connects to it while it has a remote to relay to and the hook has
  confirmed zones leading to that remote alone (see "Master side" below), and closes
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
- Several remotes: a `Host` event before each `Enter` names the machine the events
  after it are for, and the agent splits the batches; with an older agent or hook,
  batches carry no destination and the agent merges and drives only the selected
  remote (lot 5a). See "Several remotes at once".
- The master's own cursor is hidden while the pointer is away
  (`lbm-hook/src/hook/linux/evdev/hider/`): once the `Enter` is queued, and shown
  again on every way home (crossing back, no client, a write the stream refused, a
  layout without that machine, the rescue, Stop/pause/suspend/quit). On Wayland, a
  3×3 transparent layer-shell surface on the overlay layer — input region, no
  keyboard — goes under the frozen pointer and sets no cursor on `wl_pointer.enter`;
  once the compositor has configured it, the frozen pointer is moved onto it, 4
  logical pixels inside the output it left by on both axes. Moved, because a
  compositor picks the surface under the pointer when the pointer moves, not
  necessarily when a surface maps under a still one; inside, because the edge it left
  by is an outer edge of this desktop, where hot corners, screen-edge actions and
  auto-hidden panels react to a resting pointer. Coming home starts from where it
  left, not from there. On X11, XFixes hides the cursor. A thread of the hook owns the
  connection; the routing thread never waits for it. If the hook dies, the
  compositor (or the X server) drops what it asked for, and the cursor is back. When
  the display server cannot be reached or lacks layer-shell, one line in the log says
  so and the cursor stays visible.

### The Windows master

The same rules, fed by `WH_MOUSE_LL` and `WH_KEYBOARD_LL` instead of devices
(`hook::ll_master`, tested on every system against the real engine; `hook::windows::master`
is its system side). Nothing of it runs while the input endpoint has no client: the
endpoint says when its client changes (`Outlet::on_change`), and only then is the
keyboard hook installed and the window that hides the cursor made.

- **Motion.** A low-level mouse hook reports the cursor plus the displacement the
  system is about to apply, accelerated, and past the desktop's edge when the mouse
  pushes on it (tried on the vm124: x = -5 at the left edge, 5763 at the right one).
  The engine therefore crosses to another machine's zone as it crosses between
  monitors. While the pointer is away every move is swallowed, which leaves the cursor
  where it is, and the report is read as `report - cursor`: the system's acceleration
  is kept, which Raw Input's counts would not have. The cursor is first frozen where it
  left, then moved under the remote's hider window (`remote/windows/hider.rs`, reused),
  in the middle of the monitor, so that no edge bounds the next report. A pointer that
  reports absolute positions (a tablet, a remote desktop) has no displacement to give
  and drives the other machine badly.
- **Buttons and wheels** away go to the rules one event at a time: taken, swallowed;
  left for this machine (the release of a button pressed here), passed on.
- **Keys.** "Grabbed" means the keyboard hook swallows every key for the stream, from
  the moment no key pressed here is held (a modifier held while crossing keeps the
  keyboard here until it is released, as on Linux). Keys are read by scancode and
  `LLKHF_EXTENDED` (`remote::keymap::linux_key`, the inverse of the remote's table); a
  key injected by virtual key alone, with no scancode, by its virtual key. A release
  whose press went over there is swallowed here unless the system has that key down.
- **What no hook sees.** Ctrl+Alt+Del and Win+L take the input to the secure desktop:
  the desktop switch brings the pointer home, whose `Leave` releases over there what
  was pressed. A window of a higher integrity level in the foreground receives the
  input without a non-elevated hook seeing any of it (tried on the vm124: no move, no
  key); one that takes the focus while the pointer is away brings it home at the next
  tick. Starting LittleBigMouse elevated (`StartElevated`) gives the hook the same
  level; then it sees it all (tried).
- **A timer** of the hook's thread, every 100 ms while the pointer is away only, lets a
  rescue held without autorepeat fire and a client that went be noticed without a move.
  The hook's watchdog, which reinstalls a hook Windows dropped when the cursor moves
  with no move counted, still watches while the pointer is away — a dropped hook would
  leave the mouse moving this machine's cursor while the keyboard drives the other —
  and only lets pass the cursor being where the master put it itself (onto the hider,
  back home). Reinstalling unhooks first, which brings the pointer home.
- **No keyboard hook, no crossing.** Until the keyboard hook is installed, the other
  machines' borders are walls (`Desk::can_drive`), tried again at most once a second:
  otherwise a click in a field over there followed by typing would type here.
- **Lock keys** are read with `GetKeyState` on the hooks' thread as the pointer crosses
  out (`Platform::locks`).

On a remote (`lbm-hook/src/remote`):

- A **session** runs from an `Enter` to its end: a `Leave`, the end of the stream
  (the agent went away or sent garbage), a newer client taking the endpoint, or the
  hook stopping. Before the `Enter` nothing is injected; at the end, every key and
  button the stream left pressed is released.
- A machine nobody drives shows no pointer, as with Synergy/Deskflow: at the end of
  a session, after the releases, the pointer is **parked** — once per session — and
  an `Enter` **unparks** it before its first motion.
- Windows: a parked pointer is moved to the centre of the monitor it was on and put
  on a one-pixel window of the hook's — topmost, almost transparent, never activated,
  in neither Alt-Tab nor the taskbar — over which there is no cursor. Not left where
  the master left it: that is always an edge, often a corner, where hot corners, the
  auto-hidden taskbar and the Start button react to a pointer resting. The centre of
  its own monitor is on a real screen whatever the monitors' arrangement. Nothing
  global changes, so a crash cannot leave the cursor hidden. The pointer is given
  back at once when the remote's own mouse moves (it leaves the window's one pixel,
  and the window sees it go), clicks or turns its wheel there, when a master enters
  again, and when the hook stops.
- Linux: parking does nothing visible. A uinput device cannot hide the compositor's
  cursor and there is no portable way to, and moving the pointer to a corner instead
  would set off hot corners.

## Model

- **Cross-machine identity** of a panel: EDID manufacturer code + product code +
  serial (the serial string, else the binary serial). Never the checksum or the
  interface, which differ per input. Two displays from two machines with the same
  key are **one `Monitor` with two sources**.
- **Ids of a remote display** (`lbm_layout::remote`): its source is
  `{id on the remote}@{first 8 characters of the machine id}`, never one of this
  machine's ids. A display no monitor of the layout already shows becomes a monitor of
  its own, named by its **panel key** when it has one, else by its source id. The key,
  because a hardware KVM moves a panel between machines (this one stops detecting it,
  the remote starts publishing it), and Linux already names a local monitor
  `{pnp}_{serial}`, which is that key: the panel keeps one monitor id whichever machine
  shows it, so the layout id, the profile, its placement in millimetres, its borders
  and resistance carry over. When that id is already taken — a twin of a panel already
  in the layout (same serial; Linux tells local twins apart with `@{connector}`) — the
  monitor takes its source id instead. Windows names its local monitors otherwise
  (`{pnp}{serial}_{week}_{year}_{checksum}`): a Windows master names the remote's
  monitor that way, computed from the EDID the remote sent (`RemoteMonitor::local_id`,
  `lbm_display::peer::IdForm`), then by its panel key, then by its source id. Through a
  KVM the remote reads the block Windows reads here, so the panel keeps its id too, even
  without a serial number. The panel key still decides that a panel seen by both
  machines at once is one monitor.
- **One profile file across a KVM switch**: a stored `ActiveSource` naming a source
  the layout does not hold now is ignored (the merge's choice stands), and a save keeps
  the stored sources of a monitor that the layout does not hold now when they remember
  an `InputSource`, so alternating saves on either side lose neither machine's value.
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
  is what this machine last read of that panel's VCP 0x60 (see "Input switching"),
  absent when unread or unreadable. Input batches go to the hook's input endpoint, connected at
  the first batch and again after a failure; with no connection they are dropped. The
  end of the link closes that connection: the hook releases what is held. So does a
  batch that finds the agent's queue to the hook full: it may have been a release, so
  the connection is closed after what is queued, and the hook ignores what follows
  until the master's next `Enter` — which is why the master is then told `InputReset`.
  The same holds for any batch the hook did not get: dropped for want of a connection,
  or written on a connection that ended since by no doing of the link (the hook went,
  a write failed or took longer than a second). Batches are numbered as they are
  queued, and every link's task hears the number of the latest one lost; the one whose
  master's **session** it belongs to says `InputReset`, once. A session starts at the
  first batch queued after the master's `Leave` — after an `InputReset`, only at its
  next `Enter` — and ends at its `Leave` or that `InputReset`. Nothing is said for a loss
  out of a session or at the end of the link, and what the master sends before it
  hears the `InputReset` opens no session. After losing its hook the agent tries again
  at the first batch a second later, except for a batch that carries an `Enter`: that
  one tries at once, so a hook that restarted takes the next crossing; while no hook is
  there, each crossing is answered by one `InputReset`.
- **Master side**: the latest `Displays` per remote (highest revision) are kept and
  shown through the API; the hook's batches go to the selected remote with `try_input`.
  **One remote at a time** with a hook that does not say `InputHosts` (otherwise see
  "Several remotes at once"): batches carry no destination, so exactly one linked remote
  is **selected** — the only one whose screens are merged (lot 5b) and the only one the
  stream goes to. It stays selected for as long as its link lives (another linking
  does not take the screens from under the pointer); when there is none, the linked
  remote with the lowest machine id is taken, the merge order, so the same machines give
  the same choice whatever the timing.
- **Relaying waits for the hook's zones**: the hook makes the other machines' zones
  enterable while the agent is connected to its input endpoint, and a batch carries no
  destination, so a zone of a remote no longer selected would send its `Enter`, clicks
  and keys to the one that is. Every change of the selected remote (to another, another
  link of the same one, or none) is a new **choice**, numbered anew; the connection to
  the hook ends at once, and a new one is made only once the agent's loop says the hook's
  zones are fit for that very choice: every zone the hook holds, and every one sent to it
  and not yet answered, leads to no machine but that remote. What the hook holds is known
  from its answers — each `Load` the agent sends carries an `Id`, and the `Loaded` or
  `LoadFailed` that answers it names it again, so an answer is never taken for another
  `Load`'s, not even across a reconnection (a hook that predates the `Id` names none, and
  its answers are taken in order) — and from its greeting: none, or the fingerprint of
  the zones the agent would send now. Anything else (a layout from a previous life of the
  agent, a `Load` another client sent or one the agent counted lost with its connection,
  no hook) is unknown and fit for nothing until the agent's own `Load` is confirmed. The agent never sends zones leading to another machine than the
  selected remote: a `Load` of the layout built for the previous choice, before the
  debounced rebuild for the new one, is dropped, and the rebuild sends the right one.
- **The end of a hop, on the master**: one connection to the hook's input endpoint
  relays to one remote link for its whole life. It ends when the selected remote
  changes or goes, when the hook goes, or when a batch finds the link's queue full (a
  batch is never dropped mid-session: it may be a release). The connection is then
  closed (the hook comes home and releases its keyboards) and that link is sent
  `Leave` with a send that waits for room — or ends with the link, whose end the remote
  takes the same way. No connection relays anything before that `Leave` is queued.
  An `InputReset` from the selected remote ends the hop the same way and a new
  connection is made at once: the master's hook comes home, releases its keyboards and
  shows its cursor, and the next crossing sends a fresh `Enter` that the remote takes.
- **`SelectInput`** is heard only from a paired machine linked as this one's master, and
  only for a display this machine has (its id in `Displays`, a monitor wired here); from
  anyone else it is logged and ignored. The master sends it only to a machine linked as
  its remote.
- **API** (additive to protocol 3, capability `MultiPc`):
  - `Peers` → `{ Enabled, Port, Name, MachineId (this machine's fingerprint, null before
    the first use), ListenAddress, Error, SelectedRemote (the machine id of the selected
    remote, null when none), Peers: [{ MachineId, Name, Address, Connected, Displays,
    WallpaperRevision, Role, State: "Paired" | "Pending", Problem? }] }`. `Problem`
    `{ Kind, Detail }` says why the last attempt to link with that machine failed or why
    its last link ended badly, until a link with it is up again: `Kind` is `Network`,
    `Refused` (the reason it gave), `NotConfirmed` (nobody confirmed the pairing there in
    time), `UnknownFingerprint` (another machine answers at its address; `Detail` is its
    fingerprint), `Incompatible` or `Lost`; absent for a link ended on purpose;
  - `PeerWallpapers { MachineId }` (capability `PeerWallpapers`) → `{ MachineId,
    Revision, Wallpapers: [{ Display, Image | Color }] }`, see "Wallpapers";
  - `PeerOverlay { MachineId, Bands }` (capability `PeerOverlay`) → `null`: the rulers
    to draw on that remote's screens, empty to take them down, see "Rulers on a remote's
    screens"; `OverlayError` on the remote's entry in `Peers` says why it does not show
    them;
  - `SetMultiPc { Enabled, Port?, Name? }` → the same state, once the listener is up or
    everything is closed;
  - `PeerConnect { Address }` (`host`, `host:port`, `[v6]:port`) → that machine's entry
    once it said `Hello`;
  - `PeerAccept`, `PeerRefuse`, `PeerForget` `{ MachineId }` → `null`;
  - `PeerDiscover` (capability `PeerDiscovery`) → `null`: the machines that are not
    paired are dropped from `Discovered` and the network is asked again; the state
    carries `Discovered: [{ MachineId, Name, Address, Paired }]` while discovery runs
    (absent otherwise) and `DiscoveryError` when it could not start or stopped;
  - event `{ "Event": "Peers", "Peers": <state> }` to subscribers on every change.
- `cargo run -p lbm-agent --example peer_drive -- ADDR ID_DIR [script]` is a master
  without a hook, for trying a remote by hand (see the example's header).
- `cargo run -p lbm-agent --example multi_master -- pair|drive …` is a master of several
  remotes without a hook: the manager, as the master of a hook that names its machines,
  plays that hook itself across every remote (see the example's header).

## The master's layout (lot 5b)

- **Which screens**: those of the **selected remote** (`SelectedRemote`, lot 5a), a
  machine in `peers.json` linked now as a **remote** of this one (it said `Remote` in its
  `Hello`) that has sent its `Displays`. The other linked remotes are shown in the
  Machines panel but not merged: a zone of theirs would send their input to the
  selected one. A machine this one is the remote of is never merged, nor one pending. A
  frontend reading a state without `SelectedRemote` (an older agent) merges every linked
  remote, in machine-id order, as that agent did.
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
     machine_id, name, &displays)` for the selected remote, with the `Displays` of the
     `Peers` state (`Name` is the peer's);
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
  of the monitor's sources, attached to its machine's desktop — or not attached but with
  a known input value, which the switch is to bring there) on the panel, saves, and
  hands a running engine the new zones; answered `null` at once. The input switch it
  may ask for (below) runs afterwards and is never waited for.

## Several remotes at once

A master drives every remote linked to it, each with its screens placed in the
master's layout. The pointer goes to the machine whose border it crosses, and from one
remote straight to another when their screens touch in the layout. This section is
the contract; it replaces "one remote at a time" wherever both ends support it, and
leaves that rule in force wherever one of them does not (see "Older peers" below).

### What the stream carries

- **The destination is the master's business only.** It is said on the hook → agent
  hop, and the master's agent strips it: what travels between agents, and what a
  remote's hook reads, is unchanged. A remote never learns that other remotes exist,
  so nothing is asked of it, and a remote of any version can be one of several.
- **`InputEvent::Host { id }`** (tag 8; one length byte, then the machine id in UTF-8,
  at most 255 bytes): *the events that follow, up to the next `Host`, are for the
  machine `id`*. The hook writes it before every `Enter`, and only then: a crossing is
  the only moment the destination changes. A batch may carry two, when the pointer
  goes from one remote to another within one report: `Leave` for the first, then
  `Host`, `Enter`, `Locks` and the rest for the second. The id is the zones' `Host`,
  the remote's machine id.
- **The master's agent splits** each batch at its `Host` events
  (`lbm_ipc::input::by_host`, a walk over the tags with no decoding, borrowing every
  run of events), and relays each run to its machine's link, without the `Host`, and
  without the `Locks` for a remote that does not read them, as today. A run before any
  `Host` on a connection goes nowhere: a hook that says destinations always starts with
  one.
- **The hook says what machines it may cross to**: `Command::Drivable { Hosts }`, sent
  by the agent on every hook connection after `Hello`, and again whenever the set
  changes. `Hosts` are the machine ids of the remotes linked now whose stream this
  agent relays (its own link or the remote's service). A remote's zones are in play
  only while it is in that set (`ZonesLayout` gains per-host enterability, still
  rebuilt only on change): a border to any other machine is a wall, as without a
  client. The pointer on a machine that leaves the set comes home, like a layout
  without that machine.

### Older peers

Capabilities, as for `InputLocks`:

| Hook | Agent | Result |
|---|---|---|
| says `InputHosts` in its `Hello` | sends `Drivable` | several remotes |
| says `InputHosts` | older (never sends `Drivable`) | one remote: the hook writes no `Host`, any remote zone goes to the one client, and crossing from one remote to another comes home, as today |
| older (no `InputHosts`) | newer | one remote: the agent keeps the selection below and never sends `Drivable`, which the older hook would ignore anyway (`Command::Unknown`) |

The remote side needs nothing: the network `Hello` gains no capability for this,
since nothing it carries changes. The two ends of the hook ↔ agent hop are normally
the same version; the table is for an agent updated while an older hook keeps running,
which a hook does (it outlives its agent).

### The crossing rules (`hook::crossing`)

- `locate` reports a zone of a machine other than the one being driven as an `Enter`
  of that machine, no longer `Away`; `Away` is left for a position in no zone.
- **Remote to remote**, with destinations: one batch `Leave` (for the machine left),
  `Host` (the machine entered), `Enter`, `Locks`, then the frame's events. What the
  first machine holds pressed is released there by its `Leave` (the remote's session
  rule); a key released afterwards is the release of a press that went to the first
  machine, and is not forwarded to the second, which never saw the press. The keyboards
  stay grabbed and the local cursor stays hidden; the rescue's state and the lock
  keys' alignment start afresh for the machine entered.
- Without destinations, crossing from one remote to another still comes home.
- **The pointer lock** holds the pointer on the machine it is on, as now: a report that
  would take it to a third machine leaves it where it was.
- **The rescue**, Stop, the end of the stream, the client changing and the master
  going away bring the pointer home from whichever machine it is on, with a `Leave` to
  that machine. Nothing else changes on those paths.

### The master's agent

- **No selected remote** once the hook says `InputHosts`: every paired machine linked
  as a remote that has sent its `Displays` is merged, in machine-id order, and is
  drivable. `SelectedRemote` is then absent from the `Peers` state: a frontend that
  finds no `SelectedRemote` merges every linked remote in that order, which is what an
  older frontend already does for an older agent, so both build the agent's layout.
  The legacy selection, its numbered choices and the "zones fit for the choice" gate
  stay for an older hook, and only for it.
- **One connection** to the hook's input endpoint, kept while any remote is drivable.
  Its batches are split; each machine's runs go to that machine's current target (its
  link, or its service while one carries its stream, as `retarget` decides today, per
  machine). The connection is no longer tied to one link: a remote linking or leaving
  changes `Drivable` and the targets, not the connection.
- **The machine the pointer is on** (`driven` in the state, the tray's dimmed icon, the
  Machines page) is the `Host` of the last `Enter` relayed with no `Leave` since; the
  end of the connection clears it.
- **The end of a hop is per machine**, and brings the pointer home only from the
  machine it is on:
  - a run that finds its link's queue full, or no target (a race with the link's end):
    that machine's stream is broken until its next `Enter` — its runs are dropped — and
    it is sent `Leave` with a send that waits (it ends at the latest with the link). If
    the pointer is on that machine, the connection is closed and made again, which
    brings it home, as `restart` does today;
  - `InputReset` from a remote, its service's reset, and `InjectionBlocked` (blocked)
    while the pointer is on that remote: the same restart. From a remote the pointer
    is not on, nothing: its next `Enter` starts a new session there anyway;
  - a remote's link ending: it leaves `Drivable` and the targets. If the pointer is on
    it, the hook brings it home when it reads the new set, and the restart above
    covers the moment before; elsewhere nothing is felt.
  - a remote **reached another way** under the same machine id — its Windows service
    taking the stream from its agent, or giving it back: it stays in `Drivable`, only
    its relay changes. If the pointer is on it, its session is the old relay's: the
    master's agent closes the connection to the hook (the pointer comes home, the hook
    ungrabs and releases nothing more over there) and tells the **old relay** `Leave`,
    with a send that waits, before any new connection relays anything; a run read off
    the hook meanwhile for that machine is refused rather than sent to the new relay.
    The new relay hears of that machine only from the next crossing's `Enter`. So a key
    held at that moment is released by the old relay and never reaches the new one. An
    old relay already gone (the service's link ended) has its own end as `Leave`. If the
    pointer is not on it, nothing is felt: no session is open there, and its next
    `Enter` goes to the new relay;
  - every `Leave` the agent owes a remote — a run it had no room for, a relay left
    behind — is queued before anything of a later session to that remote: a new
    `Enter` for it waits for that `Leave`, and a new connection to the hook waits for
    all of them.
- **Two masters driving one remote** (#816) are untouched: that is the remote's side,
  which this changes in nothing. Each master still learns from the remote's
  `InputReset` that its session ended, and brings its pointer home if it was there.

### What follows the pointer, and what does not

- **Lock keys**: sent after each `Enter`, so each remote entered is aligned on the
  master's state, per remote capability as now.
- **UIPI blocking** (Windows remotes): per remote, as now (`InjectionBlocked` on each
  remote's entry). It ends the hop only for the remote the pointer is on.
- **The indicator**: the master shows which remote has the input; each remote shows
  that it is driven, as now.
- **The clipboard** is per link and does not follow the pointer: every remote announces
  its copies to the master, the master announces its own to every remote. A copy made on
  one remote is pasted on the master, **not on another remote**: the master does not
  pass on what it received (no echo), and relaying it would make the master the source
  of a copy it does not hold. Re-announcing a remote's offer to the other remotes, the
  master fetching on their behalf, is left for later.
- **Rulers on a remote's screens**, wallpapers, input switching: already per remote.
- **Profiles**: the set of monitors includes every merged remote, so a second remote
  linking selects (or creates, off) another profile, as the first one did.

## Wallpapers

The master's map draws every screen over the wallpaper it shows. This machine's are read
by the window itself; a remote's are on another desktop, so the remote sends them.

- **Reading, on the remote** (`lbm-agent/src/peers/wallpapers.rs`): the agent asks its
  desktop what each output shows — read-only, the same reader the window uses for its own
  map (`lbm_wallpaper::desktop::query`: Plasma's containments through `evaluateScript`,
  `IDesktopWallpaper::GetWallpaper` per monitor on Windows; a solid colour is an answer) —
  matches the outputs to its `Displays` by geometry, and draws each picture as the
  desktop does (Fill, Fit, Stretch, Center, Span and Tile, with Plasma's one-axis tiles),
  no wider than **384 px**. On Linux a display's bounds are logical pixels, so it is drawn
  at its output's scale, as the window's own preview does: a centred or tiled picture keeps
  the share of the screen it has there. Windows' bounds are physical already (scale 1). The desktop is behind the agent's `Desktop` trait, so a test
  hands it a fake and never reads the developer's.
- **Bounds**: a JPEG of at most **96 KiB** per display (quality 80, then 65 and 50, then
  half and a quarter of the size; past that, the average colour), and at most **512 KiB**
  of pictures per message (the largest pictures become their colour first). KDE's own
  wallpapers come out at 8–21 KiB on a 16:9 screen and 22–47 KiB on a portrait one; three
  screens make a message of 60–110 KB, base64 and JSON included, well within the 1 MiB of
  a control frame. They travel remote → master, against the input stream, so they never
  queue ahead of a click.
- **When**: at link-up (what was last read, then a fresh reading), after every display
  change of the remote (its rebuild), and when the remote's desktop says its wallpaper
  changed (below); under a new `Revision` only when the pictures differ; and only while
  a master is linked — a machine nobody watches reads nothing. **Never on a timer.**
- **The desktop saying so** (`lbm-agent/src/wallpaper_watch.rs`, `Desktop::watch`):
  events only, installed when a master links and taken down when the last one leaves or
  the mode is turned off — a machine with no master linked holds no descriptor, thread
  or registration for this. An event only means "read again": the reading decides, and
  the same pictures again are sent to nobody.
  - **Plasma**: inotify on the directory of `plasma-org.kde.plasma.desktop-appletsrc`
    (`$XDG_CONFIG_HOME`), which plasmashell replaces by a rename when a containment's
    wallpaper is configured. The session (`XDG_CURRENT_DESKTOP`) says this is Plasma,
    not the file: a Plasma that has not saved its configuration yet is watched for the
    first time it does. A file that is a link (a dotfile manager's) is followed to
    where it points, followed again when the link appears or changes, and still
    watched there while the target is missing. plasmashell's `wallpaperChanged` D-Bus signal is not used:
    it is emitted by its `setWallpaper` method only, not by the settings dialog. What
    Plasma changes without writing its configuration — the next picture of a slide
    show — is not seen, nor read: the reader reads that configuration.
  - **Windows**: the `WM_SETTINGCHANGE` broadcast naming `SPI_SETDESKWALLPAPER`, on the
    hidden window the agent already has (`winwatch.rs`), and a
    `RegNotifyChangeKeyValue` on `HKCU\Control Panel\Desktop` for what changes the
    wallpaper without a broadcast (a slide show does not send one). A thread asleep on
    two handles while a master is linked; none otherwise. Its first registration asks
    for a reading, for what changed between the reading taken at the link and the
    thread being armed. Per-virtual-desktop
    wallpapers and Spotlight are not known to be covered.
  - **Other desktops** (GNOME, a bare compositor): no watch. Their wallpapers are not
    read either; were they, a change would follow at the next link or display event.
  - **One reading per change**: a change is a burst of events (a write then a rename, a
    broadcast and a registry write). The reading waits for 400 ms of quiet after the
    last event, 2 s after the first at most — a delay armed by the event, not a timer
    that runs.
- **On the master**: the latest per remote (highest revision on the link) is kept and
  stamped with a counter of the master's own (`WallpaperRevision` in the `Peers` state,
  `0` while there is none); the same pictures again keep their stamp. The pictures are
  never in the `Peers` state or event, which every frontend gets at every change: a
  frontend asks `PeerWallpapers` when the stamp changes.
- **In the window** (`lbm-app/src/remote_wallpapers.rs`): one request per stamp, the
  pictures decoded once and uploaded as textures once per change. The stamps are one
  agent's: after a new connection to the agent (which may have restarted and stamps from
  zero again) every remote's pictures are asked for once more whatever their stamp, the
  old ones shown meanwhile. A screen whose active
  source is a remote's is drawn over that remote's picture of that display; every other
  one keeps this desktop's — so a panel two machines share shows the wallpaper of the
  machine it shows.

## Shared clipboard

Off by default, on each machine and each way (`ClipboardSend`, `ClipboardReceive` in
`multi-pc.json` and `SetMultiPc`); the state says it under `Clipboard { Send, Receive,
MaxBytes, Active, Problem }`. The agent owns it (`lbm-agent/src/peers/clipboard.rs`,
`lbm-agent/src/clipboard/`): it has nothing to do with the input path, and the hook never
sees it.

- **Announced at once, carried on demand.** Every copy is announced to every linked peer
  that shares, formats only (`ClipboardOffer`, numbered by the sender; an empty list says
  the sender holds nothing it shares any more). The receiver takes the system clipboard
  over *without content* — delayed rendering on Windows, a data source under Wayland —
  and asks for a format (`ClipboardRequest`, with a transfer number of its own) the first
  time an application reads it. The answer is a bulk transfer, or `ClipboardAbort` with a
  reason (the clipboard changed since, the bound, sending off). The clipboard does not
  follow the pointer: syncing at the crossing would tie it to the input path, and the
  on-demand transfer already keeps an unread copy off the network.
- **Files** (palier 3): `Files` is `text/uri-list` (and `x-special/gnome-copied-files`
  when offered) under Wayland, `CF_HDROP` on Windows. The offer lists every entry,
  folders walked, links not followed (`Files: [{ Path: "photos/2026/a.jpg", Size,
  Folder }]`, `/`-separated, a folder before its content), within a bound for the whole
  set (`ClipboardFilesMaxBytes`, 4 GiB by default, 10 000 entries). Both systems' file
  clipboards hold paths, not content, so the receiver fetches every file first, range by
  range (`ClipboardRequest { Item, Offset, Length }`, at most 4 MiB a range, the next one
  asked once the last is written), into a folder of its own under the reception folder,
  created new for that paste (a name taken, by another agent sharing the folder say, is
  never reused, and an abandon removes only the folder it created)
  (`ClipboardFolder`, `Downloads/LittleBigMouse` by default), then hands the application
  those paths; it copies from there. A name that would land outside that folder (`..`,
  an absolute path, a drive) refuses the whole offer, and so do two entries that would
  land on one file (`Photo.jpg` and `photo.jpg` where case is ignored, or after names
  Windows cannot hold were made fit); and since the volume of the reception folder may
  ignore case where the system does not (a share), every entry is created only if absent,
  and one landing on another fails the paste rather than overwrite it. A name that is not
  UTF-8 is not offered: the wire carries names as text; a narrow `CF_HDROP` is decoded in
  the ANSI code page, and one it cannot decode is not read. Every range is read from the
  very file that was listed — not a link since, the same file (device and inode on Unix,
  volume and file index on Windows), size and time, checked before and after the read —
  or the paste fails rather than mix two versions; the size announced is that very
  file's. A copy emptied or replaced while its files are listed is never offered, nor
  one whose listing outlived the clipboard closing. A list too large for one control message is not offered,
  and the copier's state says so. Paths go between the system clipboard and the sharing
  NUL-separated, exactly as the system has them. The fetch starts at the first read and is not tied to it: the reader may
  give up first (a Qt application under Wayland after one second without data, Windows
  after 30 s), and a paste once the files are here gets them at once. A fetch is stopped,
  and what it wrote removed, when the offer is replaced or its machine leaves; what a fetch
  finished stays, as a paste may be copying from it.
- **Formats** (palier 1): `Text` (UTF-8, `\n` line ends; `CF_UNICODETEXT` with `\r\n` on
  Windows, `text/plain;charset=utf-8` and its aliases under Wayland) and `Png` (`image/png`;
  on Windows the registered `PNG` format when the source offers one, else `CF_DIB`
  converted, and both offered when pasting). Unknown formats are ignored, so newer
  formats are additive.
- **Never ahead of input.** Bulk pieces wait in a queue of their own that the link's
  writer serves last: after the keepalive, after control and input, after reading (two
  peers that both wrote without reading would wait on each other for good). The kernel
  send buffer of every link is cut to 64 KiB (`lbm_net::link::SEND_BUFFER`): what the
  writer has handed the socket leaves in order, and a buffer of the system's usual
  megabytes would put that much bulk ahead of the next click. On a wired network that is
  more than is in flight; on a slow Wi-Fi it caps a transfer's pace and costs input a few
  milliseconds at worst.
- **Two copies at once.** Every offer says the last serial its sender heard from the
  receiver (`Seen`) and whether it takes offers at all (`Receiving`). An offer from a
  machine that takes them, whose `Seen` is older than the receiver's own last
  announcement to it, crossed that announcement: the machine with the lower id keeps its
  copy, the other takes it, the same rule on both sides, so that both paste the same
  thing rather than each holding the other's and neither able to serve its own.
- **Only what was announced is read.** The clipboard numbers each copy as it reports it
  (Wayland: each selection the compositor announces; Windows: the clipboard's sequence
  number), and a read names the copy it is for: a newer one, or the same one marked not
  to be monitored since, is refused (`the clipboard changed since`), checked with the
  Windows clipboard open so that nothing comes between the check and the read.
- **Relayed between remotes.** Remotes are linked to their master only, never to each
  other, so the master relays. The offer of a remote it holds is announced in turn to
  its other remotes, as an offer of the master's own under the number its clipboard
  holds it by, never to the remote it came from. A read of it there is passed on to that
  remote under a transfer number of the master's — whole formats and file ranges alike —
  and what comes back goes on to the asker piece by piece as it arrives, never buffered
  whole; an abort from either end is passed to the other, and so is a link ending. The
  relay takes the master's own options (receiving to hold the offer, sending to pass it
  on) and its bound, on top of those of both remotes. When the relayed offer is let go of
  (its remote copied a secret, stopped sending, left, or another copy replaced it), the
  remotes it went to are told with an empty offer under its number, which one holding a
  newer offer of the master's ignores. Two copies at once follow the same rule as above:
  a relayed offer is the master's as far as the remote knows. Only a master relays, and
  only from a remote to a remote: with two masters, each relays for itself, and since no
  remote relays, no offer goes round.
- **No echo.** A clipboard never reports a change it made itself (Windows: it is the
  owner; Wayland: its source offers the private `application/x-littlebigmouse-remote`
  type), so a received copy is never announced back.
- **Guards.** A bound per transfer, 64 MiB by default (`ClipboardMaxBytes`), held by both
  sides; a transfer quiet for 20 s is given up; a peer's offer is let go of when its link
  ends, so a paste finds nothing rather than waits. Not announced: what a password manager
  marks (`x-kde-passwordManagerHint` under KDE, `ExcludeClipboardContentFromMonitorProcessing`
  on Windows). An offer held on Windows is kept out of the clipboard history and the
  cloud clipboard, whose readers would otherwise fetch every copy at once.
- **Per system.** Wayland: `ext-data-control-v1` on a connection of the agent's own
  (KWin 6 has it, GNOME has neither it nor `wlr-data-control`, and the state then says
  so). Windows: a message-only window with `AddClipboardFormatListener`; `WM_RENDERFORMAT`
  waits up to 25 s for the content (Windows itself gives up at 30 s), the reading
  application waiting with it. A content handed over whole: a Qt application under
  Wayland gives up after one second without data, so a large picture over a slow network
  may fail to paste. X11 and the
  primary selection are not shared.
## Rulers on a remote's screens

"Rulers on screens" (Location mode) draws on every screen a ruler graduated in
millimetres from the selected panel. The remote's screens get theirs too, each at its
place and its physical size. The selected panel may be a remote's as well as one of
the master's: its millimetres are the layout's either way, and every screen, the
master's and the remotes', measures it.

- **Computed by the master**, from its layout: `lbm-app` takes, per remote machine on
  the map, the layout with every other machine's sources off their desktop
  (`remote_rulers::wanted`) and runs the same `lbm_overlay::scene::from_layout` as for
  its own screens. The millimetres (where the zero falls, the measured panel's length,
  each screen's physical length) are the master's; the pixels, the scale and the
  output's name are those the remote reported in `Displays`. The window sends a set
  only when it changed (`PeerOverlay`): a drag sends one per redraw, a still layout
  none.
- **Checked and drawn by the remote**: every band must be a ruler on one of its attached
  screens as they are now — same output, same pixel rectangle, same scale — or the whole
  set is refused with `OverlayFailed` (a master working from an older report must not
  draw across the wrong place). Drawn by `lbm_overlay::Standalone`, **passive**: the
  pointer and the keyboard go through them (the remote has no editor to hand a drag to;
  the rulers are dragged from the master's own screens). Windows: layered windows owned
  by a hidden window, on a thread waiting on its message queue only. Linux: the
  layer-shell or X11 surfaces of the same helper process the editor uses (the agent run
  as `--screen-overlay-host`), whose two-second lease the thread renews every 500 ms
  while — and only while — rulers are shown.
- **Never left behind**: they go when the master sends an empty set (rulers off,
  Escape, another mode, the remote leaving the map), when the master's frontend that
  asked goes (the agent takes down, on each remote, the rulers that connection wrote
  last; a window turning its rulers off or closing never takes down those another
  window wrote since), when the link ends,
  when the mode is turned off on either side, when the remote's screens change (checked
  again; the master sends a new set from the new `Displays`), and when the surfaces
  report a display change. The surfaces and their thread exist only while shown.
- A remote without `RemoteOverlay` is sent nothing; `OverlayError` says it cannot show
  rulers.
- `examples/peer_drive` asks for them (`rulers N`, `norulers`); `examples/peer_remote`
  runs a remote's multi-PC node alone, with real rulers, beside a user's running agent.

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

What lot 8 does (`lbm-agent/src/input_switch.rs`, `vcp.rs`):

- **Opt-in**: nothing is read from or written to a monitor unless VCP control is enabled
  in that machine's layout (Settings), as for the property panel; each machine decides
  for its own monitors. Off — the default — a shared panel switches routing only.
- **DDC/CI**: Linux through `ddcutil`, Windows through dxva2 (the monitor of the layout
  found by its device id below its adapter source, then the `HMONITOR` showing that
  source and its physical monitor). Every request runs outside the agent's loop, one at
  a time with the property panel's, under a deadline.
- **Learning**, after every rebuild (a display event) and when a master links — never on
  a timer: a machine reads 0x60 of its panels, and a reading is the value of **the source
  shown**. A monitor that keeps both inputs alive is attached to both machines whatever
  it shows, so a machine cannot tell its own input from the other's by being attached:
  - a panel two machines know is read only while this machine's source is the active
    one, and the value goes to that source;
  - a remote reads every panel it has while a master is linked (it cannot know which
    are shared) and publishes the readings as `InputSource`; its master merges the
    remotes' screens **without** those values and adopts one only for a remote source
    its panel shows (active and attached);
  - a learnt value is saved at once (`InputSource` on the source in the store): it is
    needed precisely when its source is not shown and cannot be read. A failed read
    (no DDC/CI, a KVM, no ddcutil) leaves the learnt value as it was and publishes
    none, with one log line per monitor and error.
- **Switching**, on `SetActiveSource` only. Nothing is written when the chosen source
  has no known value. Otherwise, on the machine that tries, 0x60 is read first and
  written only when it differs (two sources on one input — a KVM — write nothing):

  | The master's own DDC link to the panel | Who writes |
  |---|---|
  | answers, already showing the value | nobody |
  | answers, other input | the master |
  | none (no local source, VCP control off), does not answer, or the write fails | the remote, asked with `SelectInput`: the machine the panel showed, else the one chosen, else any other with a source on it |
  | as above, and no remote linked for that panel | nobody: logged; a choice of a source not attached is given up for the rule's |

  The remote does the same read-then-write on its side and answers `InputSwitched`;
  what it shows next comes with the display events that follow.
- **No fight with the attachment rule**: a write is only ever caused by `SetActiveSource`,
  never by a display event. A source chosen while not attached is kept as the choice
  across the rebuilds the switch sets off (the panel detaching here before the other
  machine reports it), until it is attached — then the rule keeps it — or the user
  chooses again, or nobody could switch.
- **The frontend** is told of a failed switch by the agent's state:
  `InputSwitchFailures: [{ Monitor, Source, Value, Error, Remote?, RemoteError? }]` —
  why this machine could not (DDC/CI's words), and, when a remote was asked, which one
  and why it could not either. In the state rather than an event because the switch is
  carried out after `SetActiveSource` is answered; a panel's entry, and what was asked
  about it, goes when it is chosen again, switched or leaves the layout (plugged back, it starts with
  nothing said). The window shows it on the panel's badge and under its
  switch. A remote that sends no `InputSwitched` (an older one) leaves nothing to show.

## Lots

1. Contracts: this document, `lbm_ipc::input`.
2. `lbm-net`: identity, TLS, pairing store, frames, control messages.
3. Model: host on sources, cross-machine key, merge, layout id, zones with host.
4. Remote injection in `lbm-hook`: input endpoint, `SendInput`, uinput.
5. Agent: peer manager, API, input relay, remote displays in the world.
6. Master hook (evdev): remote sink, keyboard grab, coming home.
7. egui: Machines panel, remote monitors, source switch.
8. Input switching over VCP 0x60 (Windows DDC included).
9. The remotes' wallpapers on the master's map.
