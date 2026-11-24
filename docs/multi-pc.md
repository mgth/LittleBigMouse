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
<<<<<<< HEAD
  - `ClipboardOffer { Serial, Formats }`, `ClipboardRequest { Serial, Format, Transfer }`,
    `ClipboardAbort { Transfer, Reason }` — either way, between peers that both announced
    `Clipboard`: see "Shared clipboard"
=======
  - `Overlay { Revision, Bands: [Band] }` — master → remote: the rulers to draw on the
    remote's screens, in place of any drawn before; empty takes them down (see "Rulers
    on a remote's screens"). Sent only to a remote whose `Hello` lists the capability
    `RemoteOverlay`; added after protocol 1 without a new version
  - `OverlayFailed { Revision, Error }` — remote → master: the rulers of `Revision`
    are not shown, refused or taken down, and why
>>>>>>> 9d66615f22a09df6d561b0734b2a0b9008f02f82
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

On the master (lot 6, evdev backend only; the rules are in
`lbm-hook/src/hook/crossing/`, platform-neutral, and what evdev adds to them in
`lbm-hook/src/hook/linux/evdev/pipeline.rs`):

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
- One remote: batches carry no destination; the agent merges and drives only the
  selected remote (lot 5a).
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
  (`{pnp}{serial}_{week}_{year}_{checksum}`), so a Windows master does not get that
  carry-over.
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
  **One remote at a time**: batches carry no destination, so exactly one linked remote
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
  - event `{ "Event": "Peers", "Peers": <state> }` to subscribers on every change.
- `cargo run -p lbm-agent --example peer_drive -- ADDR ID_DIR [script]` is a master
  without a hook, for trying a remote by hand (see the example's header).

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

<<<<<<< HEAD
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
=======
## Rulers on a remote's screens

"Rulers on screens" (Location mode) draws on every screen a ruler graduated in
millimetres from the selected panel. The remote's screens get theirs too, each at its
place and its physical size.

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
>>>>>>> 9d66615f22a09df6d561b0734b2a0b9008f02f82

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
