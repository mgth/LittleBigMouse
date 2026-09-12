# The C#↔Rust format contract

Two implementations of the same documents, in two languages, neither generated from the
other: **the contract is duplicated by hand.** This directory holds the golden payloads
that keep the two copies honest, and this file says who decides what.

It used to be a *wire* contract — the C# UI spoke to the hook over a local endpoint, in
XML. It is not any more. Phase 4 put the agent between them (the UI speaks JSON to the
agent, `LittleBigMouse.Ui.Avalonia/Remote/AgentClient.cs`), and phase 5 made the two
Rust processes speak serde types to each other
(`rust/crates/lbm-ipc/src/protocol.rs`) — a protocol with no second implementation to
disagree with, and therefore nothing to pin here.

What is left is the two documents both languages still read or write, and will until the
C# side goes: the **layout** (`<ZonesLayout>`) and the **probe report**
(`<ProbeReport>`).

## Who is authoritative

Authority runs **per document**, and it is not symmetric.

### The layout: C# is the producer of record

`<ZonesLayout>` is produced by
`LittleBigMouse.Core/LittleBigMouse.Zones/IXmlSerializable.cs` (`ZoneSerializer`).

The critical property: **the XML element and attribute names are nowhere written down.**
`ZoneSerializer.Serialize` takes `x => x.SomeMember` lambdas and uses
`typeof(T).Name` and `member.Name` as the XML names. Renaming a C# property renames a
wire attribute, silently, with no compiler error on either side — the daemon just starts
reading a default. There is no schema to disagree with; the C# type *is* the schema.

So the authoritative definitions are:

| Element | Authoritative C# definition |
|---|---|
| `<ZonesLayout>` | `LittleBigMouse.Zones/ZonesLayout.cs` — `Serialize()` |
| `<Zone>` | `LittleBigMouse.Zones/Zone.cs` — `Serialize()` |
| `<ZoneLink>` | `LittleBigMouse.Zones/ZoneLink.cs` — `Serialize()` |
| `<Rect>` | `ZoneSerializer.Serialize(Rect)` |

The Rust reader (`rust/crates/lbm-zones/src/layout.rs`, `rust/crates/lbm-zones/src/xml.rs`)
is a **follower**. If it disagrees with a golden, the bug is in Rust.

### The probe report: Rust is the producer of record

| Element | Authoritative Rust definition |
|---|---|
| `<ProbeReport>` | `rust/crates/lbm-engine/src/probe.rs` — `to_xml` |

The C# reader (`ProbeReport.TryParse`) is a **follower**. If it disagrees with a golden,
the bug is in C#.

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
  daemon-to-ui/          owned by Rust (wire_goldens.rs)
    probe-report.xml               the report for layout-v5.6-current.xml
```

Read by:

* `rust/crates/lbm-hook/tests/wire_goldens.rs`
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

## Enum spellings on the wire

The wire spellings are not always the names used in the code, and this is the part that
has already drifted once.

| Enum | Wire values | Notes |
|---|---|---|
| `Algorithm` | `Strait`, `Cross` | Case-sensitive. `CornerCrossing` accepted as an alias for `Cross`. See below. |
| `Priority` / `PriorityUnhooked` | `Idle`, `Below`, `Normal`, `Above`, `High`, `Realtime` | Unknown → `Normal` |
| `ProbeEdge/@Side` | `Left`, `Top`, `Right`, `Bottom` | |

> **`Algorithm` used to be spelled five different ways in this repository.** The wire
> value is `Cross` — that is what every shipped release offers in
> `LbmOptionsViewModel.AlgorithmList`, so it is the only spelling a real user
> configuration has ever contained. But the doc comments on `ILayoutOptions.Algorithm` and
> `LbmOptions.Algorithm` said `CornerCrossing`, the persistence fixtures under
> `TestData/Persistence/` stored `CornerCrossing`, `LocationControlViewModelDesign` used
> lowercase `strait`/`cross`, and `ZonesLayout.Algorithm`'s own initialiser was the
> lowercase `strait`. None of those were accepted by the daemon: they all landed on
> `Strait`, silently, since an unknown algorithm is not an error.
>
> Four now agree on `Strait`/`Cross`; the fifth is gone entirely, because
> `LocationControlViewModelDesign` turned out to be dead — nothing bound its list and its
> runtime counterpart never declared it — so the whole class went rather than being kept
> in step. The single remaining producer is `LbmOptionsViewModel.AlgorithmList`, pinned by
> `AlgorithmChoiceTests`. The daemon additionally **tolerates
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

There is **no version number on these documents**, and adding one would not help: a
layout written by any version has to be readable by any other. Compatibility is therefore
per-field, and the goldens are how it is checked.

(The agent↔hook protocol, which is not this, *does* carry one —
`lbm_ipc::protocol::PROTOCOL` — because those two processes do not have to agree about a
document, they have to agree about each other, and one of them outlives the other across
an upgrade.)

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

## What is deliberately NOT covered here

* **The agent↔hook protocol.** It has one implementation, in
  `rust/crates/lbm-ipc/src/protocol.rs`, and its own round-trip tests. A contract needs
  two parties who could disagree; that one has none.
* **The persistence format** (`layouts/*.json`, the registry). Different contract,
  different goldens — `LayoutPersistenceGoldenTests` and `TestData/Persistence`. It
  shares the `Algorithm` value with this one, which is exactly how the spelling drift got
  in.
* **`LoadFromFile`** — accepted by the daemon, not emitted by the current UI.
* **`Connected`** — a `LittleBigMouseEvent` member no daemon frame maps to; it is raised
  UI-side on socket connect.
