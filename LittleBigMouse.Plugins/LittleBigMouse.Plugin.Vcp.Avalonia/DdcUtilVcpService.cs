#nullable enable
using System.IO;
using System.Text.RegularExpressions;
using HLab.Sys.Monitors;
using HLab.Sys.Windows.MonitorVcp;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Linux: map a layout monitor to its DDC/CI I2C bus through sysfs and talk to it
/// with ddcutil. The bus comes from the DRM connector directory — DisplayPort
/// exposes the AUX-channel adapter as an i2c-N subdirectory, HDMI/DVI as the ddc
/// symlink (`ddcutil detect` reports the same buses but needs ~5s to probe every
/// adapter of every GPU; the sysfs scan is instant). Matching is by connector
/// name — on Linux <see cref="PhysicalMonitor.DeviceId"/> IS the connector
/// ("DP-1") — with the EDID serial as tie-breaker and fallback.
/// </summary>
public partial class DdcUtilVcpService : IVcpService
{
    record DdcDisplay(int Bus, string Connector, string Serial, string Manufacturer);

    readonly object _lock = new();
    readonly Dictionary<string, VcpControl> _controls = [];

    public VcpControl? GetControl(PhysicalMonitor monitor)
    {
        lock (_lock)
        {
            if (_controls.TryGetValue(monitor.Id, out var cached)) return cached;

            // scan every call: it is cheap and follows hot-plug
            var display = Find(Detect(), monitor);
            if (display is null) return null;

            var control = new VcpControl(monitor.Id, display.Manufacturer,
                new DdcUtilVcpTransport(display.Bus), VcpExpendMonitor.Worker);

            _controls[monitor.Id] = control;
            return control;
        }
    }

    static DdcDisplay? Find(List<DdcDisplay> displays, PhysicalMonitor monitor)
    {
        // two GPUs can expose the same connector name: prefer the serial match
        var byConnector = displays.Where(d => d.Connector == monitor.DeviceId).ToList();
        if (byConnector.Count > 0)
            return byConnector.FirstOrDefault(d => d.Serial == monitor.SerialNumber) ?? byConnector[0];

        return displays.FirstOrDefault(d =>
            !string.IsNullOrEmpty(monitor.SerialNumber) && d.Serial == monitor.SerialNumber);
    }

    static List<DdcDisplay> Detect()
    {
        var displays = new List<DdcDisplay>();

        const string drm = "/sys/class/drm";
        if (!Directory.Exists(drm)) return displays;

        foreach (var dir in Directory.EnumerateDirectories(drm))
        {
            var connectorMatch = ConnectorRegex().Match(Path.GetFileName(dir));
            if (!connectorMatch.Success) continue;

            try
            {
                if (File.ReadAllText(Path.Combine(dir, "status")).Trim() != "connected") continue;

                var bus = FindBus(dir);
                if (bus is null) continue;

                string serial = "", manufacturer = "";
                var edidPath = Path.Combine(dir, "edid");
                if (File.Exists(edidPath))
                {
                    var bytes = File.ReadAllBytes(edidPath);
                    if (bytes.Length >= 128)
                    {
                        var edid = EdidParser.Parse(dir, bytes);
                        serial = edid.SerialNumber is { Length: > 0 } sn ? sn : edid.Serial ?? "";
                        manufacturer = edid.ManufacturerCode ?? "";
                    }
                }

                displays.Add(new DdcDisplay(bus.Value, connectorMatch.Groups[1].Value, serial, manufacturer));
            }
            catch (Exception ex)
            {
                // connector without usable sysfs entries
                Console.Error.WriteLine($"VCP: skipping {dir}: {ex.Message}");
            }
        }

        return displays;
    }

    static int? FindBus(string connectorDir)
    {
        // DisplayPort: DDC/CI runs over the AUX channel, exposed as an i2c-N subdirectory
        foreach (var sub in Directory.EnumerateDirectories(connectorDir))
        {
            var name = Path.GetFileName(sub);
            if (name.StartsWith("i2c-") && int.TryParse(name[4..], out var aux)) return aux;
        }

        // HDMI/DVI: the ddc symlink points to the adapter
        var ddc = Path.Combine(connectorDir, "ddc");
        var target = File.ResolveLinkTarget(ddc, returnFinalTarget: true);
        if (target is not null)
        {
            var name = Path.GetFileName(target.FullName);
            if (name.StartsWith("i2c-") && int.TryParse(name[4..], out var bus)) return bus;
        }

        return null;
    }

    [GeneratedRegex(@"^card\d+-(.+)$")]
    private static partial Regex ConnectorRegex();
}
