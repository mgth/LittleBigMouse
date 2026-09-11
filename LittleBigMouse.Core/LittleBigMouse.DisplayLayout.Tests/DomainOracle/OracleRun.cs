using System.Globalization;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Platform.Linux;
using LittleBigMouse.Plugins.Persistence;
using LittleBigMouse.Ui.Avalonia.Main;

namespace LittleBigMouse.DisplayLayout.Tests.DomainOracle;

/// <summary>
/// Runs one scenario through the production Linux pipeline and renders what came out.
/// <para>
/// Nothing here re-implements a step: the layout is built by
/// <see cref="LinuxLayoutFactory.Populate"/> (the body of <c>LinuxLayoutFactory.Create</c>
/// minus the enumeration and the wallpaper read), with the production options class
/// (<see cref="LbmOptions"/>, compiled into this assembly from the UI's source) and the
/// production JSON store in a scratch directory. The only substitutions are the two
/// platform hooks that would touch the machine running the test — see
/// <see cref="OraclePersistence"/>.
/// </para>
/// </summary>
static class OracleRun
{
    /// <summary>The expected/ files, relative path → content (LF line endings).</summary>
    public static SortedDictionary<string, string> Outputs(OracleInput input)
    {
        var work = Path.Combine(Path.GetTempPath(), "lbm-domain-oracle", Guid.NewGuid().ToString("N"));
        var config = Path.Combine(work, "config");
        var data = Path.Combine(work, "data");
        Directory.CreateDirectory(config);
        Directory.CreateDirectory(data);

        var culture = CultureInfo.CurrentCulture;
        var uiCulture = CultureInfo.CurrentUICulture;
        try
        {
            // The one environmental input the domain reads: culture-sensitive string
            // comparison orders the sources, hence the zones. Pinned, so the corpus does
            // not depend on the locale of whoever regenerates it.
            CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;
            CultureInfo.CurrentUICulture = CultureInfo.InvariantCulture;

            SeedStore(input, config);
            var excludedFile = Path.Combine(data, "Excluded.txt");
            if (input.Excluded is { } excluded) File.WriteAllLines(excludedFile, excluded);

            var store = new JsonLayoutStore(config);
            var persistence = new OraclePersistence(store, excludedFile);

            using var layout = new MonitorsLayout(new LbmOptions());
            LinuxLayoutFactory.Populate(
                layout,
                input.Displays.Select(d => d.ToLinuxMonitor()).ToList(),
                persistence);

            var outputs = new SortedDictionary<string, string>(StringComparer.Ordinal)
            {
                ["layout.json"] = Json(Layout(layout, RelativeStorePath(config, store.LayoutPath(layout.Id)))),
                ["zones.xml"] = layout.ComputeZones().Serialize() + "\n",
                ["pixel-locations.json"] = Json(PixelLocations(layout)),
            };

            // Last: a save only flips Saved flags on the model, but nothing above should
            // have to reason about that.
            persistence.Save(layout);
            outputs["saved-store.json"] = Json(SavedStore(config));

            return outputs;
        }
        finally
        {
            CultureInfo.CurrentCulture = culture;
            CultureInfo.CurrentUICulture = uiCulture;
            try { Directory.Delete(work, true); } catch { /* scratch space */ }
        }
    }

    /// <summary>
    /// <c>LinuxLayoutPersistence</c> is exactly <c>LayoutPersistence(new JsonLayoutStore())</c>
    /// with the base hooks; this is the same engine over a store in a scratch directory, with
    /// the two hooks that read the machine pinned: the excluded-processes file lives in the
    /// scratch data directory instead of ~/.local/share, and the process counts as not
    /// elevated whoever runs the suite (CI runners often are). Autostart keeps the base
    /// no-ops, which is what Linux ships.
    /// </summary>
    sealed class OraclePersistence(ILayoutStore store, string excludedFile) : LayoutPersistence(store)
    {
        protected override bool IsElevated => false;

        protected override string ExcludedListFile() => excludedFile;
    }

    //==================//
    // Input            //
    //==================//

    static void SeedStore(OracleInput input, string config)
    {
        if (input.Store is not { } files) return;

        foreach (var (relative, content) in files)
        {
            if (Path.IsPathRooted(relative) || relative.Split('/').Contains(".."))
                throw new InvalidDataException($"store path must stay inside the store: {relative}");

            var path = Path.Combine(config, relative.Replace('/', Path.DirectorySeparatorChar));
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);

            // A JSON string is raw file text (a file that is not valid JSON); anything else
            // is the document itself. The layout of the text is irrelevant to the reader.
            File.WriteAllText(path, content is JsonValue value && value.TryGetValue<string>(out var raw)
                ? raw
                : content?.ToJsonString(Indented) ?? "null");
        }
    }

    static string RelativeStorePath(string config, string path)
        => Path.GetRelativePath(config, path).Replace(Path.DirectorySeparatorChar, '/');

    //==================//
    // layout.json      //
    //==================//

    static JsonObject Layout(MonitorsLayout layout, string storeFile) => new()
    {
        ["Id"] = layout.Id,
        ["StoreKey"] = LayoutStoreKey.For(layout.Id),
        ["StoreFile"] = storeFile,
        ["Saved"] = layout.Saved,
        ["PrimaryMonitor"] = layout.PrimaryMonitor?.Id,
        ["PrimarySource"] = layout.PrimarySource?.Id,
        ["PhysicalBounds"] = Rect(layout.PhysicalBounds),
        ["MaxEffectiveDpiX"] = Number(layout.MaxEffectiveDpiX),
        ["MaxEffectiveDpiY"] = Number(layout.MaxEffectiveDpiY),
        ["Options"] = Options(layout.Options),
        ["Monitors"] = Array(layout.PhysicalMonitors
            .OrderBy(m => m.Id, StringComparer.Ordinal)
            .Select(Monitor)),
    };

    /// <summary>
    /// Every option the model carries, except the two platform-hook results (LoadAtStartup,
    /// Elevated), the constant LoopAllowed and the Saved flag.
    /// </summary>
    static JsonObject Options(ILayoutOptions o) => new()
    {
        ["Enabled"] = o.Enabled,
        ["AllowOverlaps"] = o.AllowOverlaps,
        ["AllowDiscontinuity"] = o.AllowDiscontinuity,
        ["Algorithm"] = o.Algorithm,
        ["MinimalEdgeOverlap"] = Number(o.MinimalEdgeOverlap),
        ["MaxTravelDistance"] = Number(o.MaxTravelDistance),
        ["MinimalMaxTravelDistance"] = Number(o.MinimalMaxTravelDistance),
        ["FreelookCheckInterval"] = Number(o.FreelookCheckInterval),
        ["FreelookEnabled"] = o.FreelookEnabled,
        ["LoopX"] = o.LoopX,
        ["LoopY"] = o.LoopY,
        ["AdjustPointer"] = o.AdjustPointer,
        ["AdjustSpeed"] = o.AdjustSpeed,
        ["IsUnaryRatio"] = o.IsUnaryRatio,
        ["BorderValues"] = o.BorderValues,
        ["RescueShortcut"] = o.RescueShortcut,
        ["Priority"] = o.Priority,
        ["PriorityUnhooked"] = o.PriorityUnhooked,
        ["AutoUpdate"] = o.AutoUpdate,
        ["HomeCinema"] = o.HomeCinema,
        ["Pinned"] = o.Pinned,
        ["StartMinimized"] = o.StartMinimized,
        ["StartElevated"] = o.StartElevated,
        ["HideTrayIcon"] = o.HideTrayIcon,
        ["DebugTools"] = o.DebugTools,
        ["ExperimentalFeatures"] = o.ExperimentalFeatures,
        ["VcpControl"] = o.VcpControl,
        ["ShowMonitorActionWarning"] = o.ShowMonitorActionWarning,
    };

    static JsonObject Monitor(PhysicalMonitor m) => new()
    {
        ["Id"] = m.Id,
        ["DeviceId"] = m.DeviceId,
        ["SerialNumber"] = m.SerialNumber,
        ["Model"] = new JsonObject
        {
            ["PnpCode"] = m.Model.PnpCode,
            ["PnpDeviceName"] = m.Model.PnpDeviceName,
            ["Logo"] = m.Model.Logo,
            ["PhysicalSize"] = Size(m.Model.PhysicalSize),
        },
        ["Placed"] = m.Placed,
        ["ExcludedFromLayout"] = m.ExcludedFromLayout,
        ["BordersCustomized"] = m.BordersCustomized,
        ["Borders"] = new JsonObject
        {
            ["Left"] = Number(m.Borders.Left),
            ["Top"] = Number(m.Borders.Top),
            ["Right"] = Number(m.Borders.Right),
            ["Bottom"] = Number(m.Borders.Bottom),
        },
        ["EffectivePhysicalSize"] = Size(m.EffectivePhysicalSize),
        ["PhysicalRotated"] = Size(m.PhysicalRotated),
        ["DepthRatio"] = Ratio(m.DepthRatio),
        ["DepthProjectionUnrotated"] = Size(m.DepthProjectionUnrotated),
        ["DepthProjection"] = Projection(m.DepthProjection),
        ["Diagonal"] = Number(m.Diagonal),
        ["ActiveSource"] = m.ActiveSource?.Source.Id,
        ["BorderResistance"] = new JsonObject
        {
            ["Left"] = Sections(m.BorderResistance.Left),
            ["Top"] = Sections(m.BorderResistance.Top),
            ["Right"] = Sections(m.BorderResistance.Right),
            ["Bottom"] = Sections(m.BorderResistance.Bottom),
        },
        ["Sources"] = Array(m.Sources.Items
            .OrderBy(s => s.Source.Id, StringComparer.Ordinal)
            .Select(Source)),
    };

    /// <summary>
    /// A display source and the ratios derived from it. RealDpiAvg is left out on purpose:
    /// it goes through Math.Pow, which the C runtimes do not all round alike, and it is
    /// RealDpi's quadratic mean — derivable, and not worth a golden that differs per OS.
    /// </summary>
    static JsonObject Source(PhysicalSource s) => new()
    {
        ["Id"] = s.Source.Id,
        ["DeviceId"] = s.DeviceId,
        ["DeviceName"] = s.Source.DeviceName,
        ["DisplayName"] = s.Source.DisplayName,
        ["SourceName"] = s.Source.SourceName,
        ["InterfacePath"] = s.Source.InterfacePath,
        ["SourceNumber"] = s.Source.SourceNumber,
        ["Primary"] = s.Source.Primary,
        ["AttachedToDesktop"] = s.Source.AttachedToDesktop,
        ["Orientation"] = s.Source.Orientation,
        ["DisplayFrequency"] = s.Source.DisplayFrequency,
        ["InPixel"] = Rect(s.Source.InPixel.Bounds),
        ["EffectiveDpi"] = Ratio(s.Source.EffectiveDpi),
        ["DpiAwareAngularDpi"] = Ratio(s.Source.DpiAwareAngularDpi),
        ["RawDpi"] = Ratio(s.Source.RawDpi),
        ["InDip"] = Rect(s.InDip.Bounds),
        ["RealPitch"] = Ratio(s.RealPitch),
        ["Pitch"] = Ratio(s.Pitch),
        ["RealDpi"] = Ratio(s.RealDpi),
        ["Dpi"] = Ratio(s.Dpi),
        ["DipToPixelRatio"] = Ratio(s.DipToPixelRatio),
        ["PixelToDipRatio"] = Ratio(s.PixelToDipRatio),
        ["PhysicalToPixelRatio"] = Ratio(s.PhysicalToPixelRatio),
        ["MmToDipRatio"] = Ratio(s.MmToDipRatio),
    };

    static JsonArray Sections(BorderSide side) => Array(side.Sections.Items.Select(s => (JsonNode?)new JsonObject
    {
        ["From"] = Number(s.From),
        ["To"] = Number(s.To),
        ["Move"] = Number(s.Move),
        ["MoveBlock"] = s.MoveBlock,
        ["Drag"] = Number(s.Drag),
        ["DragBlock"] = s.DragBlock,
    }));

    static JsonObject Size(IDisplaySize s) => new()
    {
        ["X"] = Number(s.X),
        ["Y"] = Number(s.Y),
        ["Width"] = Number(s.Width),
        ["Height"] = Number(s.Height),
        ["LeftBorder"] = Number(s.LeftBorder),
        ["TopBorder"] = Number(s.TopBorder),
        ["RightBorder"] = Number(s.RightBorder),
        ["BottomBorder"] = Number(s.BottomBorder),
    };

    static JsonObject Projection(IDisplaySize s)
    {
        var o = Size(s);
        o["Bounds"] = Rect(s.Bounds);
        o["OutsideBounds"] = Rect(s.OutsideBounds);
        return o;
    }

    /// <summary>
    /// A ratio, or null for the ones that have no value yet — several are derived from the
    /// layout's primary source, and a layout without a primary never publishes them.
    /// </summary>
    static JsonNode? Ratio(IDisplayRatio? r) => r is null
        ? null
        : new JsonObject
        {
            ["X"] = Number(r.X),
            ["Y"] = Number(r.Y),
        };

    static JsonObject Rect(Rect r) => new()
    {
        ["X"] = Number(r.X),
        ["Y"] = Number(r.Y),
        ["Width"] = Number(r.Width),
        ["Height"] = Number(r.Height),
    };

    //==================//
    // pixel-locations  //
    //==================//

    /// <summary>
    /// What "apply the layout to the system" would ask for, without asking: the solver's
    /// answer with and without the Wayland scale adjustment, keyed by source.
    /// </summary>
    static JsonArray PixelLocations(MonitorsLayout layout) => Array(new[] { false, true }.Select(adjustScale =>
        (JsonNode?)new JsonObject
        {
            ["AdjustScale"] = adjustScale,
            ["Placements"] = Array(layout.ComputePixelLocationsFromPhysical(adjustScale)
                .OrderBy(kv => kv.Key.Id, StringComparer.Ordinal)
                .Select(kv => (JsonNode?)new JsonObject
                {
                    ["Source"] = kv.Key.Id,
                    ["X"] = Number(kv.Value.PixelBounds.X),
                    ["Y"] = Number(kv.Value.PixelBounds.Y),
                    ["Width"] = Number(kv.Value.PixelBounds.Width),
                    ["Height"] = Number(kv.Value.PixelBounds.Height),
                    ["Scale"] = kv.Value.Scale is { } scale ? Number(scale) : null,
                })),
        }));

    //==================//
    // saved-store.json //
    //==================//

    /// <summary>
    /// The store directory after the save, in <c>input.json</c>'s own <c>store</c> shape.
    /// Every file is listed, the untouched ones included: what a user is left with is the
    /// whole directory, and JsonLayoutStore merges into models.json rather than replacing it.
    /// <para>
    /// The excluded-processes file is deliberately NOT recorded, although the save rewrites
    /// it: its default entries are per-OS (<c>ExcludedProcessDefaults.All</c>), and a corpus
    /// whose expected output depends on the machine that ran it is not an oracle. The list
    /// itself is covered by <see cref="LayoutPersistenceGoldenTests"/>; what a scenario can
    /// still show here is the version counter the top-up writes into options.json.
    /// </para>
    /// </summary>
    static JsonObject SavedStore(string config)
    {
        var files = new JsonObject();
        foreach (var path in Directory.GetFiles(config, "*", SearchOption.AllDirectories)
                     .Select(p => RelativeStorePath(config, p))
                     .Order(StringComparer.Ordinal))
        {
            var text = File.ReadAllText(Path.Combine(config, path.Replace('/', Path.DirectorySeparatorChar)));
            JsonNode? content;
            try { content = JsonNode.Parse(text); }
            catch (JsonException) { content = text; }
            files[path] = content;
        }

        return files;
    }

    //==================//
    // Encoding         //
    //==================//

    static readonly JsonSerializerOptions Indented = new() { WriteIndented = true };

    /// <summary>
    /// Two-space indentation, LF, final newline, and no escaping of '+' and friends: ids are
    /// "+"-joined, and a golden nobody can read is a golden nobody reviews. Numbers are
    /// System.Text.Json's shortest round-trip form.
    /// </summary>
    static readonly JsonSerializerOptions Output = new()
    {
        WriteIndented = true,
        NewLine = "\n",
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    static string Json(JsonNode node) => node.ToJsonString(Output) + "\n";

    /// <summary>
    /// A double as a JSON number, or — JSON having none — NaN and the infinities as the
    /// strings "NaN", "Infinity" and "-Infinity". A degenerate monitor (no size) does produce
    /// them, and freezing them is the point.
    /// </summary>
    static JsonNode Number(double value) => double.IsFinite(value)
        ? JsonValue.Create(value)
        : JsonValue.Create(value.ToString(CultureInfo.InvariantCulture))!;

    static JsonArray Array(IEnumerable<JsonNode?> items) => new(items.ToArray());
}
