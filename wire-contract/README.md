# The UI↔daemon wire contract

The Avalonia UI (C#) and the hook daemon (Rust) are separate processes that exchange
length-prefixed UTF-8 XML over a per-user local endpoint. Neither side generates the
other's types: **the contract is duplicated by hand in two languages.** This directory
holds the golden payloads that keep the two copies honest, and this file says who
decides what, and what to do when a message changes.

## The transport, in one paragraph

One frame = a little-endian `u32` byte length, then that many bytes of UTF-8. Max 1 MiB.
It is **not** line-oriented — piping a bare `<CommandMessage .../>` into the socket makes
the daemon read `<Com` as a length and hang up. Endpoint: a named pipe per Windows
session, a Unix socket under `$XDG_RUNTIME_DIR` on Linux; `LBM_HOOK_ENDPOINT` overrides
both, for tests. Framing lives in `LittleBigMouse-Hook-Rust/src/ipc/framing.rs` and
`LittleBigMouse.Ui.Avalonia/Remote/LocalIpcClient.cs`.

Transport behaviour (duplex, reconnection, malformed frames) is tested in
`LittleBigMouse-Hook-Rust/tests/wire_contract.rs`. This directory is about the *payloads*
that ride it.

## Who is authoritative

Authority runs **per direction**, and it is not symmetric.

### UI→daemon: C# is the producer of record

`<CommandMessage>` and its `<ZonesLayout>` payload are produced by
`LittleBigMouse.Core/LittleBigMouse.Zones/IXmlSerializable.cs` (`ZoneSerializer`).

The critical property: **the XML element and attribute names are nowhere written down.**
`ZoneSerializer.Serialize` takes `x => x.SomeMember` lambdas and uses
`typeof(T).Name` and `member.Name` as the XML names. Renaming a C# property renames a
wire attribute, silently, with no compiler error on either side — the daemon just starts
reading a default. There is no schema to disagree with; the C# type *is* the schema.

So the authoritative definitions are:

| Wire element | Authoritative C# definition |
|---|---|
| `<CommandMessage>` | `LittleBigMouse.Zones/CommandMessage.cs` |
| `<ZonesLayout>` | `LittleBigMouse.Zones/ZonesLayout.cs` — `Serialize()` |
| `<Zone>` | `LittleBigMouse.Zones/Zone.cs` — `Serialize()` |
| `<ZoneLink>` | `LittleBigMouse.Zones/ZoneLink.cs` — `Serialize()` |
| `<Rect>` | `ZoneSerializer.Serialize(Rect)` |

The Rust reader (`src/zones/layout.rs`, `src/zones/xml.rs`, `src/ipc/protocol.rs`) is a
**follower**. If it disagrees with a golden, the bug is in Rust.

### daemon→UI: Rust is the producer of record

`<DaemonMessage>` frames and the `<ProbeReport>` document are produced by
`LittleBigMouse-Hook-Rust/src/ipc/protocol.rs` and `src/engine/probe.rs`.

| Wire element | Authoritative Rust definition |
|---|---|
| `<DaemonMessage>` | `src/ipc/protocol.rs` — the `pub const`s and builder fns |
| `<ProbeReport>` | `src/engine/probe.rs` — `to_xml` |

The C# readers (`DaemonMessage.TryParse`, `ProbeReport.TryParse`) are **followers**. If
they disagree with a golden, the bug is in C#.

## The corpus

Both test suites read these files **from the source tree**, not from a build-output copy.
That is the point: there is one set of bytes, and each side is tested against the other's
actual output rather than against its own idea of it.

```
goldens/
  ui-to-daemon/          owned by C#   (WireContractGoldenTests)
    layout-v5.2.3.xml              frozen: what v5.2.3 emitted
    layout-v5.5.2.xml              frozen: what v5.5.2 emitted
    layout-v5.6-current.xml        generated from ZoneSerializer
    layout-future-unknown-fields.xml   unknown attrs/elements from a newer UI
    layout-unknown-enum-values.xml     unknown Priority / Algorithm
    command-{run,stop,quit,shortcut,load}.xml
  daemon-to-ui/          owned by Rust (wire_goldens.rs)
    events.txt                     one frame per line, every event the daemon emits
    probe-report.xml               the report for layout-v5.6-current.xml
```

Read by:

* `LittleBigMouse-Hook-Rust/tests/wire_goldens.rs`
* `LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests/WireContractGoldenTests.cs`

### The version goldens are not invented

`layout-v5.2.3.xml` and `layout-v5.5.2.xml` reproduce the shapes those releases actually
serialized, recovered from the tags (`git show v5.2.3:.../ZoneLink.cs`). They are
**frozen** — a released version cannot retroactively change what it sent. The deltas they
lock:

| Field | v5.2.3 | v5.5.2 | v5.6+ |
|---|---|---|---|
| `ZonesLayout/@Virtual` | — | ✓ | ✓ |
| `ZonesLayout/@FreelookCheckInterval`, `@FreelookEnabled` | — | ✓ | ✓ |
| `ZonesLayout/@RescueShortcut` | — | — | ✓ |
| `Zone/@DeviceId` | — | ✓ | ✓ |
| `ZoneLink/@MoveBlock`, `@DragResistance`, `@DragBlock` | — | — | ✓ |

## Compatibility rules

These are what the tests enforce, in both directions.

**Backward (old UI → new daemon).** A missing attribute must fall back to the value that
*reproduces the old behaviour*, not to a type default that happens to be convenient. The
worked example is `DragResistance`: before the move/drag split one `BorderResistance`
governed both modes, so its absence falls back to `BorderResistance` — defaulting to
`0.0` would silently unblock every dragged crossing on a pre-5.6 layout.

**Forward (new UI → old daemon).** Unknown attributes and unknown child elements are
ignored, never fatal. A rejected layout leaves the user with a confined cursor and no
configuration at all, which is strictly worse than an ignored field.

**Unknown enum values** fall back to the documented default (`Priority` → `Normal`,
`Algorithm` → `Strait`). Unknown *commands* are surfaced as `Command::Unknown(name)` and
logged, never guessed at.

**Unknown events go the other way: they are REJECTED.** `DaemonMessage.TryParse` returns
false rather than mapping an unrecognised event onto a known one, so a newer daemon
paired with an older UI degrades to "state unchanged" instead of to a wrong state.

## Enum spellings on the wire

The wire spellings are not always the names used in the code, and this is the part that
has already drifted once.

| Enum | Wire values | Notes |
|---|---|---|
| `Algorithm` | `Strait`, `Cross` | Case-sensitive. `CornerCrossing` accepted as an alias for `Cross`. See below. |
| `Priority` / `PriorityUnhooked` | `Idle`, `Below`, `Normal`, `Above`, `High`, `Realtime` | Unknown → `Normal` |
| `Command` | `Listen`, `Load`, `LoadFromFile`, `Run`, `Stop`, `State`, `Probe`, `Shortcut`, `Quit` | |
| `Event` | `Running`, `Stopped`, `Paused`, `Dead`, `SettingChanged`, `DesktopChanged`, `DisplayChanged`, `FocusChanged`, `Suspended`, `Resumed`, `Loaded`, `LoadFailed`, `Probed`, `Rescued`, `ShortcutUnavailable` | `SettingsChanged` accepted as a legacy alias; `<State>` accepted in place of `<Event>` |
| `ProbeEdge/@Side` | `Left`, `Top`, `Right`, `Bottom` | |

> **`Algorithm` used to be spelled four different ways in this repository.** The wire
> value is `Cross` — that is what every shipped release offers in
> `LbmOptionsViewModel.AlgorithmList`, so it is the only spelling a real user
> configuration has ever contained. But the doc comments on `ILayoutOptions.Algorithm` and
> `LbmOptions.Algorithm` said `CornerCrossing`, the persistence fixtures under
> `TestData/Persistence/` stored `CornerCrossing`, and `LocationControlViewModelDesign`
> used lowercase `strait`/`cross`. None of those three were accepted by the daemon: they
> all landed on `Strait`, silently, since an unknown algorithm is not an error.
>
> All four now agree on `Strait`/`Cross`, and the daemon additionally **tolerates
> `CornerCrossing` as an alias** for `Cross`, so a hand-edited config or a migration that
> trusts an old doc comment does what it says instead of silently degrading. The alias is
> a safety net, not a second blessed name — the UI must keep emitting `Cross`.
>
> Pinned by `cross_is_the_wire_value_and_corner_crossing_is_tolerated_as_an_alias`
> (Rust) and `AlgorithmWireSpellingsAreTheOnesTheDaemonUnderstands` (C#). Note the alias
> is exact: `cross` and `cornercrossing` in lowercase are still `Strait`.

Also note `Strait` is a misspelling of `Straight`, kept deliberately: the value is written
verbatim into saved layouts, so renaming it would reset the algorithm of every existing
configuration. Only the daemon's internal mode is spelled correctly (`Mode::Straight`).

## Procedure when a message changes

There is **no protocol version number** on the wire, and adding one would not help: the
two ends are shipped together but *run* in whatever pairing the user's install leaves
behind (a daemon survives a UI upgrade, and `Current.xml` replays commands written by an
older UI at boot). Compatibility is therefore per-field, and the goldens are how it is
checked.

### Adding a field (the common case)

1. Add the property and include it in the relevant `Serialize()` — remember the XML name
   is the C# member name.
2. Read it on the Rust side. **Choose the absent-value fallback deliberately** and write
   the reason in a comment: it is the behaviour every existing user gets.
3. Regenerate the C#-owned goldens and read the diff:
   ```
   LBM_UPDATE_GOLDEN=1 dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests \
       --filter FullyQualifiedName~WireContractGoldenTests
   ```
4. Add an assertion to `wire_goldens.rs` for the new field, and one to the *frozen*
   version goldens' tests proving the fallback (they must keep passing untouched — if you
   had to edit `layout-v5.2.3.xml`, you have broken compatibility, not fixed a fixture).
5. Run both suites (below).

### Adding or changing an event

1. Add the constant or builder in `src/ipc/protocol.rs`.
2. Add the case to `DaemonMessage.TryParse` **and** the enum member in
   `LittleBigMouseEvent`. An event the UI does not know is dropped silently.
3. Add the frame to the `frames` array in `daemon_event_frames_match_the_golden`, then:
   ```
   LBM_UPDATE_GOLDEN=1 cargo test --test wire_goldens
   ```
4. `EveryDaemonEventGoldenParsesToAKnownEvent` will fail until the UI learns it. That
   failure is the point.

### Renaming or removing a field

Don't, unless you also ship a migration on both sides. A rename is indistinguishable on
the wire from "the old side stopped sending it", so every existing user silently gets the
absent-value fallback. If you must: keep reading the old name on the Rust side for at
least one release, and leave the frozen version goldens alone.

### Regenerating everything

```
# C#-owned (ui-to-daemon)
LBM_UPDATE_GOLDEN=1 dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests \
    --filter FullyQualifiedName~WireContractGoldenTests
# Rust-owned (daemon-to-ui) — cold-start safe, one pass
cd LittleBigMouse-Hook-Rust && LBM_UPDATE_GOLDEN=1 cargo test --test wire_goldens
```

Then **read the diff before committing.** A changed line in `ui-to-daemon/` is a daemon
that no longer understands what the UI sends; a changed line in `daemon-to-ui/` is a UI
that no longer understands what the daemon reports.

### Running the checks

```
cd LittleBigMouse-Hook-Rust && cargo test
dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests
```

## What is deliberately NOT covered here

* **`Current.xml` replay.** The recovery file holds serialized `CommandMessage`s and is
  covered by `DaemonProtocolTests` (atomic write, `.bak`, `MarkStopped` stripping `Run`).
* **The persistence format** (`layouts/*.json`, the registry). Different contract,
  different goldens — `LayoutPersistenceGoldenTests` and `TestData/Persistence`. It
  shares the `Algorithm` value with this one, which is exactly how the spelling drift got
  in.
* **`LoadFromFile`** — accepted by the daemon, not emitted by the current UI.
* **`Connected`** — a `LittleBigMouseEvent` member no daemon frame maps to; it is raised
  UI-side on socket connect.
