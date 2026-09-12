using System.Globalization;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Platform.Linux;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.DisplayLayout.Tests.DomainOracle;

/// <summary>
/// The C# twin of <c>lbm-agent --dump-displays</c> (v6 plan, phase 2): what the production
/// discovery finds on the machine running it, and the monitor ids the layout model gives
/// that. The Rust port must print the same values on the same machine — the check the
/// recorded corpus cannot make, since it starts after discovery.
/// <para>
/// Nothing is re-implemented. On Linux the outputs, the backend name and the display
/// signature come from <see cref="LinuxLayoutFactory"/>, the plug signature from
/// <see cref="DrmEdidReader"/>, the ids from <see cref="LinuxLayoutMapping.AddMonitor"/> and
/// <c>ComputeId</c>; the outputs are written in the shape of the domain oracle's
/// <c>input.json</c>. On Windows the monitor devices come from
/// <see cref="SystemMonitorsService"/>, the layout from <c>WindowsLayoutBuilder.UpdateFrom</c>
/// over a store that loads nothing, the signature from
/// <see cref="MonitorDeviceHelper.DisplaySignature"/>. The culture is pinned to the invariant
/// one, which the display signatures format their numbers with, and which the
/// culture-sensitive sorts (monitor devices by physical id, layout ids) then use.
/// </para>
/// </summary>
public static class DisplayDump
{
    public static string Linux()
    {
        var culture = CultureInfo.CurrentCulture;
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;

            // Create() is never called: no layout factory function, no persistence.
            using var factory = new LinuxLayoutFactory(() => throw new InvalidOperationException(), null!);
            var monitors = factory.QueryMonitors();

            using var layout = new MonitorsLayout(new ILayoutOptions.Design());
            foreach (var monitor in monitors) layout.AddMonitor(monitor);

            var dump = new JsonObject
            {
                ["Backend"] = factory.SourceName,
                ["Displays"] = new JsonArray(monitors.Select(m => (JsonNode?)Display(m)).ToArray()),
                ["LayoutId"] = layout.ComputeId(),
                ["Monitors"] = new JsonArray(layout.PhysicalMonitors.Select(m => (JsonNode?)new JsonObject
                {
                    ["Id"] = m.Id,
                    ["PnpCode"] = m.Model.PnpCode,
                    ["DeviceId"] = m.DeviceId,
                }).ToArray()),
                ["PlugSignature"] = DrmEdidReader.PlugSignature(),
                ["DisplaySignature"] = factory.DisplaySignature(),
            };
            return dump.ToJsonString(new JsonSerializerOptions
            {
                WriteIndented = true,
                Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            }) + "\n";
        }
        finally
        {
            CultureInfo.CurrentCulture = culture;
        }
    }

    /// <summary>One output, as <c>input.json</c> holds it (see <see cref="OracleInput"/>).</summary>
    static JsonObject Display(LinuxMonitor m) => new()
    {
        ["ConnectorName"] = m.ConnectorName,
        ["LogicalX"] = m.LogicalX,
        ["LogicalY"] = m.LogicalY,
        ["LogicalWidth"] = m.LogicalWidth,
        ["LogicalHeight"] = m.LogicalHeight,
        ["PixelWidth"] = m.PixelWidth,
        ["PixelHeight"] = m.PixelHeight,
        ["Scale"] = m.Scale,
        ["WidthMm"] = m.WidthMm,
        ["HeightMm"] = m.HeightMm,
        ["Primary"] = m.Primary,
        ["Enabled"] = m.Enabled,
        ["Orientation"] = m.Orientation,
        ["Frequency"] = m.Frequency,
        ["Edid"] = m.Edid is not { } e
            ? null
            : new JsonObject
            {
                ["ManufacturerCode"] = e.ManufacturerCode,
                ["ProductCode"] = e.ProductCode,
                ["Serial"] = e.Serial,
                ["SerialNumber"] = e.SerialNumber,
                ["Model"] = e.Model,
                ["PhysicalWidth"] = e.PhysicalWidth,
                ["PhysicalHeight"] = e.PhysicalHeight,
                ["VideoInterface"] = e.VideoInterface,
            },
    };

    /// <summary>
    /// The Windows discovery: every monitor device of the Win32 tree in enumeration order,
    /// and the layout the production builder makes of them. The thread is made per-monitor
    /// DPI aware (v2) for the duration, as the UI's manifest makes the UI: Windows
    /// virtualizes positions, modes and DPIs for an unaware test host.
    /// </summary>
    [SupportedOSPlatform("windows")]
    public static string Windows()
    {
        var culture = CultureInfo.CurrentCulture;
        var dpiContext = SetThreadDpiAwarenessContext(DpiAwarenessContextPerMonitorAwareV2);
        try
        {
            CultureInfo.CurrentCulture = CultureInfo.InvariantCulture;

            var service = new SystemMonitorsService();
            // Held for the whole dump: the layout below is built from this same tree.
            var root = service.Root!;

            using var layout = new MonitorsLayout(new ILayoutOptions.Design());
            layout.UpdateFrom(service, new NoStore());

            var dump = new JsonObject
            {
                ["DpiAwareness"] = layout.DpiAwareness.ToString(),
                ["Displays"] = new JsonArray(root.AllMonitorDevices().Select(m => (JsonNode?)WindowsDisplay(m)).ToArray()),
                ["LayoutId"] = layout.Id,
                ["Monitors"] = new JsonArray(layout.PhysicalMonitors.Select(m => (JsonNode?)LayoutMonitor(m)).ToArray()),
                ["DisplaySignature"] = MonitorDeviceHelper.DisplaySignature(),
            };
            return dump.ToJsonString(new JsonSerializerOptions
            {
                WriteIndented = true,
                Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
            }) + "\n";
        }
        finally
        {
            CultureInfo.CurrentCulture = culture;
            if (dpiContext != 0) SetThreadDpiAwarenessContext(dpiContext);
        }
    }

    /// <summary>A monitor device: its ids, its EDID, its active connection.</summary>
    static JsonObject WindowsDisplay(MonitorDevice m) => new()
    {
        ["Id"] = m.Id,
        ["PnpCode"] = m.PnpCode,
        ["PhysicalId"] = m.PhysicalId,
        ["SourceId"] = m.SourceId,
        ["InterfacePath"] = m.InterfacePath,
        ["MonitorNumber"] = m.MonitorNumber,
        ["IsSpecialized"] = m.IsSpecialized,
        ["Edid"] = m.Edid is not { } e
            ? null
            : new JsonObject
            {
                ["ManufacturerCode"] = e.ManufacturerCode,
                ["ProductCode"] = e.ProductCode,
                ["Serial"] = e.Serial,
                ["SerialNumber"] = e.SerialNumber,
                ["Model"] = e.Model,
                ["Week"] = e.Week,
                ["Year"] = e.Year,
                ["Checksum"] = e.Checksum,
                ["PhysicalWidth"] = e.PhysicalWidth,
                ["PhysicalHeight"] = e.PhysicalHeight,
                ["VideoInterface"] = e.VideoInterface,
            },
        ["ActiveConnection"] = m.ActiveConnection is not { } c
            ? null
            : new JsonObject
            {
                ["DeviceName"] = c.DeviceName,
                ["DeviceString"] = c.DeviceString,
                ["AttachedToDesktop"] = c.State.AttachedToDesktop,
                ["Adapter"] = WindowsAdapter(c.Parent),
            },
    };

    /// <summary>An adapter source, as the mapping reads it.</summary>
    static JsonObject WindowsAdapter(PhysicalAdapter a) => new()
    {
        ["DeviceName"] = a.DeviceName,
        ["DeviceString"] = a.DeviceString,
        ["Primary"] = a.Primary,
        ["EffectiveDpi"] = Xy(a.EffectiveDpi.X, a.EffectiveDpi.Y),
        ["AngularDpi"] = Xy(a.AngularDpi.X, a.AngularDpi.Y),
        ["RawDpi"] = Xy(a.RawDpi.X, a.RawDpi.Y),
        ["CurrentMode"] = a.CurrentMode is not { } mode
            ? null
            : new JsonObject
            {
                ["X"] = mode.Position.X,
                ["Y"] = mode.Position.Y,
                ["Width"] = mode.Pels.Width,
                ["Height"] = mode.Pels.Height,
                ["Orientation"] = mode.DisplayOrientation,
                ["Frequency"] = mode.DisplayFrequency,
            },
        ["Capabilities"] = new JsonObject
        {
            ["Width"] = a.Capabilities.Size.Width,
            ["Height"] = a.Capabilities.Size.Height,
            ["ResolutionWidth"] = a.Capabilities.Resolution.Width,
            ["ResolutionHeight"] = a.Capabilities.Resolution.Height,
            ["LogPixelsX"] = a.Capabilities.LogPixels.Width,
            ["LogPixelsY"] = a.Capabilities.LogPixels.Height,
        },
    };

    /// <summary>A monitor of the layout: identity, model size, place in mm, sources.</summary>
    static JsonObject LayoutMonitor(PhysicalMonitor m) => new()
    {
        ["Id"] = m.Id,
        ["PnpCode"] = m.Model.PnpCode,
        ["DeviceId"] = m.DeviceId,
        ["SerialNumber"] = m.SerialNumber,
        ["PnpDeviceName"] = m.Model.PnpDeviceName,
        ["Logo"] = m.Model.Logo,
        ["PhysicalWidth"] = m.Model.PhysicalSize.Width,
        ["PhysicalHeight"] = m.Model.PhysicalSize.Height,
        ["X"] = m.DepthProjection.X,
        ["Y"] = m.DepthProjection.Y,
        ["Sources"] = new JsonArray(m.Sources.Items.Select(s => (JsonNode?)new JsonObject
        {
            ["Id"] = s.Source.Id,
            ["DeviceId"] = s.DeviceId,
            ["SourceNumber"] = s.Source.SourceNumber,
            ["Primary"] = s.Source.Primary,
            ["AttachedToDesktop"] = s.Source.AttachedToDesktop,
            ["Orientation"] = s.Source.Orientation,
            ["InPixel"] = new JsonObject
            {
                ["X"] = s.Source.InPixel.X,
                ["Y"] = s.Source.InPixel.Y,
                ["Width"] = s.Source.InPixel.Width,
                ["Height"] = s.Source.InPixel.Height,
            },
            ["EffectiveDpi"] = Xy(s.Source.EffectiveDpi.X, s.Source.EffectiveDpi.Y),
            ["RawDpi"] = Xy(s.Source.RawDpi.X, s.Source.RawDpi.Y),
            ["InterfaceName"] = s.Source.InterfaceName,
        }).ToArray()),
    };

    static JsonObject Xy(double x, double y) => new() { ["X"] = x, ["Y"] = y };

    /// <summary>The layout store of a first run: nothing to load, nothing saved.</summary>
    sealed class NoStore : ILayoutPersistence
    {
        public bool IsLoading => false;
        public void Load(MonitorsLayout layout) { }
        public bool Save(MonitorsLayout layout) => false;
        public bool SaveEnabled(IMonitorsLayout layout) => false;
        public void SaveLive(ILayoutOptions options) { }
    }

    /// <summary><c>DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2</c>.</summary>
    const nint DpiAwarenessContextPerMonitorAwareV2 = -4;

    /// <summary>Returns the thread's previous context, 0 when it fails.</summary>
    [DllImport("user32.dll")]
    static extern nint SetThreadDpiAwarenessContext(nint dpiContext);
}
