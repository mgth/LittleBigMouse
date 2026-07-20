#nullable enable
using System;
using System.Collections.Generic;
using System.Linq;
using OneOf;

using HLab.Sys.Windows.Monitors;

using static HLab.Sys.Windows.API.ErrHandlingApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.HighLevelMonitorConfigurationApi;
using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;
using static HLab.Sys.Windows.API.MonitorConfiguration.PhysicalMonitorEnumerationApi;

namespace HLab.Sys.Windows.MonitorVcp;

/// <summary>
/// DDC/CI through the Windows monitor configuration API (dxva2). Owns the
/// physical monitor handle. Brightness/contrast/gain/drive go through the
/// high-level functions (same behavior as before the transport seam);
/// anything else through the low-level VCP request.
/// </summary>
public sealed class DxVa2VcpTransport : IVcpTransport
{
    PhysicalMonitor[] _physicalMonitors;
    readonly int? _selectedIndex;

    public DxVa2VcpTransport(MonitorDevice monitor)
    {
        var hMonitor = monitor.ActiveConnection?.Parent.HMonitor ?? IntPtr.Zero;
        _physicalMonitors = GetPhysicalMonitorsFromHMONITOR(hMonitor);
        _selectedIndex = SelectPhysicalMonitorIndex(
            _physicalMonitors.Select(p => p.szPhysicalMonitorDescription),
            [monitor.Edid?.Model, monitor.Edid?.ProductCode,
                monitor.ActiveConnection?.DeviceString, monitor.PnpCode]);
    }

    nint HPhysical => _selectedIndex is { } index && index < _physicalMonitors.Length
        ? _physicalMonitors[index].hPhysicalMonitor
        : IntPtr.Zero;

    /// <summary>
    /// Resolve one physical handle without guessing. A logical HMONITOR may front
    /// several tiled or mirrored panels; writes are disabled unless there is one
    /// handle or the device identity matches exactly one description.
    /// </summary>
    public static int? SelectPhysicalMonitorIndex(
        IEnumerable<string?> descriptions, IEnumerable<string?> identities)
    {
        var physical = descriptions.Select(NormalizeIdentity).ToList();
        if (physical.Count == 1) return 0;
        if (physical.Count == 0) return null;

        var candidates = identities.Select(NormalizeIdentity)
            .Where(value => value.Length >= 4)
            .Distinct(StringComparer.Ordinal)
            .ToList();
        var matches = physical
            .Select((description, index) => (description, index))
            .Where(item => item.description.Length >= 4 && candidates.Any(candidate =>
                item.description.Contains(candidate, StringComparison.Ordinal)
                || candidate.Contains(item.description, StringComparison.Ordinal)))
            .Select(item => item.index)
            .Distinct()
            .ToList();
        return matches.Count == 1 ? matches[0] : null;
    }

    static string NormalizeIdentity(string? value)
        => new((value ?? string.Empty).Where(char.IsLetterOrDigit)
            .Select(char.ToUpperInvariant).ToArray());

    public IReadOnlySet<byte>? GetSupportedCodes()
    {
        if (HPhysical == IntPtr.Zero) return null;
        if (!GetMonitorCapabilities(HPhysical, out var capabilities, out _)) return null;

        var codes = new HashSet<byte>();

        if (capabilities.HasFlag(MonitorCapabilities.Brightness))
            codes.Add((byte)VcpCode.Brightness);

        if (capabilities.HasFlag(MonitorCapabilities.Contrast))
            codes.Add((byte)VcpCode.Contrast);

        if (capabilities.HasFlag(MonitorCapabilities.RedGreenBlueGain))
        {
            codes.Add((byte)VcpCode.RedGain);
            codes.Add((byte)VcpCode.GreenGain);
            codes.Add((byte)VcpCode.BlueGain);
        }

        if (capabilities.HasFlag(MonitorCapabilities.RedGreenBlueDrive))
        {
            codes.Add((byte)VcpCode.RedDrive);
            codes.Add((byte)VcpCode.GreenDrive);
            codes.Add((byte)VcpCode.BlueDrive);
        }

        return codes;
    }

    public OneOf<(uint value, uint min, uint max), int> GetFeature(VcpCode code)
    {
        if (HPhysical == IntPtr.Zero) return 0;

        uint value = 0, min = 0, max = 0;

        var result = code switch
        {
            VcpCode.Brightness => GetMonitorBrightness(HPhysical, ref min, ref value, ref max),
            VcpCode.Contrast => GetMonitorContrast(HPhysical, ref min, ref value, ref max),

            VcpCode.RedGain => GetMonitorRedGreenOrBlueGain(HPhysical, 0, ref min, ref value, ref max),
            VcpCode.GreenGain => GetMonitorRedGreenOrBlueGain(HPhysical, 1, ref min, ref value, ref max),
            VcpCode.BlueGain => GetMonitorRedGreenOrBlueGain(HPhysical, 2, ref min, ref value, ref max),

            VcpCode.RedDrive => GetMonitorRedGreenOrBlueDrive(HPhysical, 0, ref min, ref value, ref max),
            VcpCode.GreenDrive => GetMonitorRedGreenOrBlueDrive(HPhysical, 1, ref min, ref value, ref max),
            VcpCode.BlueDrive => GetMonitorRedGreenOrBlueDrive(HPhysical, 2, ref min, ref value, ref max),

            _ => GetVCPFeatureAndVCPFeatureReply(HPhysical, code, out _, out value, out max),
        };

        if (result) return (value, min, max);
        return GetLastError();
    }

    public bool SetFeature(VcpCode code, uint value)
    {
        if (HPhysical == IntPtr.Zero) return false;

        return code switch
        {
            VcpCode.Brightness => SetMonitorBrightness(HPhysical, value),
            VcpCode.Contrast => SetMonitorContrast(HPhysical, value),

            VcpCode.RedGain => SetMonitorRedGreenOrBlueGain(HPhysical, 0, value),
            VcpCode.GreenGain => SetMonitorRedGreenOrBlueGain(HPhysical, 1, value),
            VcpCode.BlueGain => SetMonitorRedGreenOrBlueGain(HPhysical, 2, value),

            VcpCode.RedDrive => SetMonitorRedGreenOrBlueDrive(HPhysical, 0, value),
            VcpCode.GreenDrive => SetMonitorRedGreenOrBlueDrive(HPhysical, 1, value),
            VcpCode.BlueDrive => SetMonitorRedGreenOrBlueDrive(HPhysical, 2, value),

            _ => SetVCPFeature(HPhysical, code, value),
        };
    }

    public void Dispose()
    {
        if (_physicalMonitors is { Length: > 0 })
            DestroyPhysicalMonitors((uint)_physicalMonitors.Length, ref _physicalMonitors);
        _physicalMonitors = [];
        GC.SuppressFinalize(this);
    }

    ~DxVa2VcpTransport() => Dispose();
}
