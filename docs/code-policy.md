# Code policy

The root `.editorconfig` is the shared, deliberately small formatting contract for
LittleBigMouse. It follows the dominant style in current code: UTF-8 files, LF line
endings, a final newline, no trailing whitespace, four-space indentation, C# braces
on a new line, and `using` directives outside the namespace with `System` directives
first. Avalonia XAML and Rust also use four spaces; Rust keeps rustfmt's default
100-column target.

The policy intentionally does not prescribe naming, `var`, expression-bodied members,
file-scoped namespaces, or other semantic preferences. Those styles are mixed in the
repository, and selecting one now would create a review-obscuring migration.

## Shared C# build settings

`Directory.Build.props` enables nullable reference analysis, the SDK analyzers, and
the latest analysis level supported by the selected SDK. A project may override a
setting only when the exception is documented below.

`ImplicitUsings` remains project-local. It is enabled by many recent projects but not
by the legacy libraries, where adding global namespaces can introduce ambiguities.
It should be reconsidered after those projects have moved to the current target
framework; until then, new projects should choose it explicitly.

`TreatWarningsAsErrors` is not enabled solution-wide yet. The current warning stock is
too large for that switch to be useful. A clean project opts into the future policy by
adding:

```xml
<CodePolicyEnforceWarnings>true</CodePolicyEnforceWarnings>
```

The same policy can be tested across the solution without editing project files:

```sh
dotnet build LittleBigMouse.sln -p:CodePolicyEnforceWarnings=true
```

## Temporary baseline

The baseline is visible rather than suppressed: warnings keep appearing in normal
builds, and no broad `NoWarn` list hides new instances. On 2026-08-18, the validation
build with .NET SDK 10.0.111 completed with 611 warnings and no errors. The stock
contains nullable (`CS86xx`/`CS92xx`), Windows platform (`CA1416`), obsolete API
(`CS0618`/`SYSLIB0014`), package (`NU1510`), and Avalonia warnings. The exact count
can vary by platform, incremental-build state, and restored dependency graph.

Temporary exceptions are:

- `HLab.Sys.Monitors.Edid` explicitly disables nullable analysis in its project file.
- `HLab.Avalonia` has its own nearer `Directory.Build.props`; it already enables
  nullable analysis but does not yet inherit every root property.
- A full `dotnet format LittleBigMouse.sln --verify-no-changes` reports substantial
  pre-existing whitespace debt: the 2026-08-18 snapshot found 10,443 diagnostics in
  396 files, notably in the `HLab.Core` and `HLab.Avalonia` submodules and older tests.
  Do not normalize those trees in an unrelated change.

For day-to-day changes, verify only the C# files being changed by passing their paths
to `dotnet format --include ... --verify-no-changes`. The full solution command remains
the target and must continue to be run when changing this policy so the exception does
not become invisible.

## Removing the baseline

Migrate one project at a time:

1. Format that project's C# files in a formatting-only change, upstreaming submodule
   changes in their own repositories.
2. Fix nullable warnings first, then platform annotations and obsolete APIs, without
   adding project-wide suppressions.
3. Add `CodePolicyEnforceWarnings=true` to the clean project and keep it enabled.
4. Re-enable nullable analysis in `HLab.Sys.Monitors.Edid`, and make the nested
   `HLab.Avalonia/Directory.Build.props` import the root policy when those trees are
   clean.
5. When every solution project is opted in, make warnings-as-errors the default,
   remove `CodePolicyEnforceWarnings`, and require the full `dotnet format` verification.

Validation commands from the repository root are:

```sh
dotnet format LittleBigMouse.sln --verify-no-changes --no-restore
dotnet build LittleBigMouse.sln --no-restore
(cd LittleBigMouse-Hook-Rust && cargo fmt --check)
```

## Misspellings kept for compatibility

Two kinds of misspelling exist in the tree, and they are not treated the same way.

Purely internal ones get fixed. On 2026-08-25 the files `RectExtentions.cs`,
`MonitorExtentions.cs`, `WinApiExtentions.cs` and `DrawingContextExtention.cs` were
renamed to `Extension(s)`; only the file names were wrong, every class inside already
used the correct spelling, and the projects glob their sources, so nothing referenced
the old names. Five more remain in the `HLab.Core` submodule — `GeoExtentions.cs`
(twice), `TaskExtentions.cs`, `TextExtentions.cs`, and `StringExtentions.cs` in its
test project — and must be renamed there, in their own repository.

One is kept on purpose: the layout option `Algorithm` takes the value **`Strait`**,
a misspelling of *Straight*. The string is not an internal name. It is written verbatim
into the saved layouts (`layouts/*.json`) and into the `Algorithm` attribute of the
`ZonesLayout` XML the UI sends to the daemon, which parses it in
`LittleBigMouse-Hook-Rust/src/zones/layout.rs`. Renaming it would silently reset the
mouse-movement algorithm of every existing configuration and break the wire contract
with any daemon that was not upgraded in the same step. It stays until a migration
exists on both sides, accepting the old value on read and covered by tests. Only the
daemon's internal mode is spelled correctly (`Mode::Straight`), mapped from `Strait`
at parse time. The current value is pinned by `LayoutPersistenceGoldenTests`,
`DaemonProtocolTests` and `VirtualLayoutGuardTests` on the C# side and by
`tests/wire_contract.rs` on the Rust side.
