using System.Globalization;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Nodes;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Platform.Linux;

namespace LittleBigMouse.DisplayLayout.Tests.DomainOracle;

/// <summary>
/// The C# twin of <c>lbm-agent --dump-displays</c> (v6 plan, phase 2): what the production
/// Linux discovery finds on the machine running it, and the monitor ids the layout model
/// gives that. The Rust port must print the same values on the same machine — the check
/// the recorded corpus cannot make, since it starts after discovery.
/// <para>
/// Nothing is re-implemented: the outputs, the backend name and the display signature come
/// from <see cref="LinuxLayoutFactory"/>, the plug signature from <see cref="DrmEdidReader"/>,
/// the ids from <see cref="LinuxLayoutMapping.AddMonitor"/> and <c>ComputeId</c>. The
/// outputs are written in the shape of the domain oracle's <c>input.json</c>. The culture is
/// pinned to the invariant one, which the display signature formats its numbers with.
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
}
