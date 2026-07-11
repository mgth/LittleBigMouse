#nullable enable
using System;
using System.Collections.Generic;
using OneOf;

using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;

namespace HLab.Sys.Windows.MonitorVcp;

/// <summary>
/// Raw DDC/CI access to one physical monitor — the platform seam under
/// <see cref="VcpControl"/>. Windows goes through dxva2
/// (<see cref="DxVa2VcpTransport"/>), Linux through ddcutil
/// (<see cref="DdcUtilVcpTransport"/>).
/// </summary>
public interface IVcpTransport : IDisposable
{
    /// <summary>
    /// VCP opcodes the monitor declares in its MCCS capabilities string,
    /// or null when the capabilities probe failed (DDC/CI unreachable).
    /// </summary>
    IReadOnlySet<byte>? GetSupportedCodes();

    /// <summary>Read a feature: (current, min, max) or a native error code.</summary>
    OneOf<(uint value, uint min, uint max), int> GetFeature(VcpCode code);

    bool SetFeature(VcpCode code, uint value);
}
