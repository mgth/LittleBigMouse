#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public interface ISamsungTizenService
{
    SamsungTizenConfiguration? GetConfiguration(string monitorId);
    Task<IReadOnlyList<SamsungTizenDevice>> DiscoverAsync(CancellationToken cancellationToken = default);
    Task<SamsungTizenDevice> ProbeAsync(string ipAddress, CancellationToken cancellationToken = default);
    Task<SamsungTizenConfiguration> PairAsync(
        string monitorId,
        string ipAddress,
        string? macAddress = null,
        CancellationToken cancellationToken = default);
    void SaveManualAddress(string monitorId, string ipAddress, string macAddress);
    void SavePictureMacro(string monitorId, string sequence);
    Task SendKeyAsync(string monitorId, string key, CancellationToken cancellationToken = default);
    Task SendSequenceAsync(
        string monitorId,
        IEnumerable<(string Key, TimeSpan DelayAfter)> sequence,
        CancellationToken cancellationToken = default);
    Task PowerOnAsync(string monitorId, CancellationToken cancellationToken = default);
}
