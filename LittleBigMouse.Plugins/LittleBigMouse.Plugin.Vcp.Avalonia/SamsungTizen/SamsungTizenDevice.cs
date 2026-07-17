#nullable enable
using System.Net;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

/// <summary>Identity advertised by the local Tizen remote-control endpoint.</summary>
public sealed record SamsungTizenDevice(
    string IpAddress,
    string Name,
    string ModelName,
    string DeviceId,
    string MacAddress)
{
    public string Label => string.IsNullOrWhiteSpace(ModelName)
        ? $"{Name} — {IpAddress}"
        : $"{Name} ({ModelName}) — {IpAddress}";

    public static bool IsValidAddress(string? value)
        => IPAddress.TryParse(value, out var address)
           && address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork;
}

/// <summary>Persisted association between an EDID monitor and one Tizen network device.</summary>
public sealed class SamsungTizenConfiguration
{
    public string MonitorId { get; set; } = "";
    public string IpAddress { get; set; } = "";
    public string MacAddress { get; set; } = "";
    public string Token { get; set; } = "";
    public string DeviceId { get; set; } = "";
    public string Name { get; set; } = "Samsung display";
    public string ModelName { get; set; } = "";
    public string PictureMacro { get; set; } = "";
}
