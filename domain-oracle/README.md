# Domain oracle

What the C# domain core does, written down while it still exists.

The v6 work (`docs/v6-architecture-plan.md` on the `v6` branch) reimplements the domain
core in Rust: the layout model (`LittleBigMouse.DisplayLayout`), the zone compiler
(`LittleBigMouse.Zoning`) and the persistence engine (`LittleBigMouse.Plugins.Core/Persistence`).
The only way to know the port behaves is to compare it with what this code does on inputs
nobody can argue about. This directory is that comparison: scenarios in, recorded outputs
out, in JSON and XML only, so both languages read the same bytes.

## Who is authoritative

As long as `DomainOracleTests` runs, **the C# code is the producer of record**. A Rust
mismatch is a Rust bug, unless the scenario's `description` says the deviation is deliberate.

A C# change that makes a scenario fail is a behaviour change of the domain, wanted or not.
Either it is a regression, or it is intended and the scenario is regenerated in the same
commit, with the diff read before it is committed.

## Layout

```
domain-oracle/
  scenarios/<name>/
    input.json                what the pipeline is given
    expected/layout.json      the layout it builds
    expected/zones.xml        the zones it would send to the daemon
    expected/pixel-locations.json   what "apply layout to system" would ask for
    expected/saved-store.json the store directory after saving right away
```

The test reads the corpus from the **source tree**, never from a build-output copy, for
the same reason as `wire-contract/goldens`: the Rust port reads these very bytes. Every
file here is LF (`.gitattributes`), and the readers normalise CRLF anyway.

## `input.json`

| Member | Meaning |
|---|---|
| `description` | Why the scenario exists and what it freezes. |
| `displays` | The outputs as the Linux platform layer enumerated them, **in enumeration order** (the order is an input too). One object per output, with exactly the members of `LinuxMonitor` (`LittleBigMouse.Platform.Linux/LinuxMonitor.cs`) under the same names: `ConnectorName`, `LogicalX`/`Y`/`Width`/`Height` (the compositor's logical space, the cursor's space under Wayland), `PixelWidth`/`Height` (the mode), `Scale`, `WidthMm`/`HeightMm` (oriented, as the source reports them), `Primary`, `Enabled`, `Orientation` (0–3), `Frequency` and `Edid`. `DisplayMirrorsLinuxMonitor` fails the day the record gains or loses a member. |
| `displays[].Edid` | `null` when no EDID was matched, otherwise the parsed members `LinuxLayoutMapping.AddMonitor` reads: `ManufacturerCode`, `ProductCode` (four uppercase hex digits), `Serial` (the eight hex digits of the binary serial, bytes 15..12), `SerialNumber` and `Model` (the 0xFF / 0xFC descriptor strings, `""` when absent), `PhysicalWidth`/`PhysicalHeight` (mm, first detailed timing), `VideoInterface`. |
| `store` | The JSON store directory before the load, keyed by path relative to it (`options.json`, `models.json`, `layouts/<name>.json`), in the on-disk format of `JsonLayoutStore`. A value is the file's JSON document; a JSON string stands for raw file text (a file that is not valid JSON). `null` or absent: nothing stored. |
| `excluded` | The lines of `Excluded.txt` before the load. `null` or absent: no such file. |

`displays` and every member of a display and of its EDID are required (an EDID member may be
`null`); `description`, `store` and `excluded` may be omitted. Unknown members are rejected:
a field one reader silently defaults is a field the two languages may disagree on.

## The pipeline

`LinuxLayoutFactory.Populate` — the body of `LinuxLayoutFactory.Create` minus the two steps
that reach the machine (output enumeration before, wallpaper read after): map each output,
compute the layout id, load the store, place from the system topology (`placeAll: false`),
anchor on the primary. The options are the production `LbmOptions`, compiled into the test
assembly from the UI's source; the store is the production `JsonLayoutStore` in a scratch
directory. Two platform hooks are pinned so the result does not depend on the machine: the
excluded-processes file lives in the scratch directory, and the process counts as not
elevated. Autostart keeps the base no-ops, which is what Linux ships.

The culture is pinned to the invariant one while a scenario runs: the model orders sources
with a culture-sensitive comparison, and the zone ids follow that order. A port must
reproduce the invariant-culture order, not ordinal order (`laptop-tv` is the scenario where
the two differ).

## Expected outputs

| File | Content |
|---|---|
| `layout.json` | Layout id, store key and store file name, `Saved`, primary monitor and source, physical bounds, the options the model carries, and per monitor (ordered by id): model (PnP code, name, logo, intrinsic size), `Placed`, `ExcludedFromLayout`, borders and whether they are customised, effective and rotated sizes, depth ratio and projection (bounds and outside bounds), active source, border-resistance sections; per source (ordered by id): identity, orientation, attachment, pixel bounds, and the DPI and pitch ratios the zones and the editor use. |
| `zones.xml` | `ComputeZones().Serialize()`: the `ZonesLayout` document the UI puts in the payload of a `Load` command, without the `CommandMessage` wrapper. Only main zones appear; loop clones take part in link computation and are never serialized. |
| `pixel-locations.json` | `ComputePixelLocationsFromPhysical` with and without the Wayland scale adjustment, per source: position, size, and the scale when one is proposed. Nothing is applied. |
| `saved-store.json` | Every file of the store directory after `Save`, in `input.json`'s `store` shape, the untouched ones included (`JsonLayoutStore` merges into `models.json` rather than replacing it). |

Encoding: two-space indentation, LF, final newline, no escaping of `+` (ids are
"+"-joined). Numbers are the shortest round-trip form. JSON has no NaN or infinities, so
they are written as the strings `"NaN"`, `"Infinity"` and `"-Infinity"`; a monitor with no
surface does produce them, and freezing them is the point.

Deliberately left out: the options that are platform-hook results (`LoadAtStartup`,
`Elevated`) or constant (`LoopAllowed`); `RealDpiAvg`, a quadratic mean through `Math.Pow`
that C runtimes do not all round alike; and the rewritten `Excluded.txt`, whose default
entries are per OS (`ExcludedProcessDefaults.All`). A scenario can still show the version
counter the defaults top-up writes into `options.json`.

## Regenerating

```
LBM_UPDATE_GOLDEN=1 dotnet test LittleBigMouse.Core/LittleBigMouse.DisplayLayout.Tests \
    --filter FullyQualifiedName~DomainOracleTests
```

It rewrites every `expected/` directory and deletes outputs a scenario no longer produces.
Then read the diff before committing. A new scenario is a new directory with an
`input.json`; the next regeneration records its outputs, and the scenario must be listed
below (`ReadmeListsEveryScenario` checks it).

Real configurations make the best scenarios, but anonymise them: replace EDID serial numbers
with invented ones.

## Scenarios

| Scenario | What it freezes |
|---|---|
| `single-1080p-fresh` | One 1920×1080 output, nothing stored: the `LbmOptions` defaults every other scenario deviates from. |
| `three-screens-mixed-scale` | Three 4K screens at scales 1.95, 1.5 and 1.25 on Wayland, nothing stored: placement from the system topology across three pitches. |
| `three-screens-mixed-scale-saved` | The same screens with a saved layout: stored mm positions that placement would not compute, a primary away from the origin, per-model borders, resistance sections. |
| `six-monitors` | A real six-monitor desktop in staggered rows, two monitors sharing one model. |
| `laptop-tv` | HiDPI laptop at scale 2, primary monitor, 4K TV at scale 3 above both: four pixel densities, and the source order that differs between culture-aware and ordinal comparison. |
| `grid-2x2` | Four identical monitors in a 2×2 grid: a corner contact claims nothing. |
| `nine-monitors-long-id` | A 3×3 wall sharing one EDID serial: connector disambiguation and a 264-character id stored under a hashed key (#589). |
| `nine-monitors-long-id-saved` | The same wall with a layout stored under the hashed key. |
| `portrait-rotated` | A rotated QHD next to a landscape primary: intrinsic model size, rotation applied downstream (#511), orientation in the layout id. |
| `portrait-pre-541-store` | The same with a pre-5.4.1 store holding the rotated size: the #507 transposition back. |
| `no-edid` | Outputs with no matched EDID, one with no size at all: connector identity, the infinities of a surface-less monitor, the stored 0×0 of #419 ignored. |
| `no-outputs-fallback` | No output at all: the invented 1920×1080 `FALLBACK` monitor. |
| `disabled-output` | A monitor switched off in the display settings: no zone, stored pixel geometry restored. |
| `excluded-monitor` | A non-primary monitor excluded from the layout (#504): attached, but no zone. |
| `loop-xy` | LoopX and LoopY on: clones in the link computation, absent from the payload. |
| `border-sections` | Per-monitor borders and several sections per edge (plain, move-blocked, drag-blocked): how sections fold into links. |
| `cross-algorithm` | Cross stored with a shorter MaxTravelDistance: the reach of the link compiler. |
| `disabled-layout` | A layout stored with the engine off: `Enabled` carried back unchanged by a save. |
| `negative-positions` | A primary at the bottom right: outputs at negative coordinates, re-anchored at 0,0 mm. |
| `v5.2-pre-sections-store` | A 5.2 store (one resistance per edge, missing options): the section migration and today's defaults. |
| `excluded-defaults-topup` | A store from before the per-OS exclusion defaults: the one load that writes, and the version it records. |
