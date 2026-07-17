#nullable enable
namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public interface IHisenseVidaaService
{
    HisenseVidaaConfiguration? GetConfiguration(string monitorId);
    Task<IReadOnlyList<HisenseVidaaDevice>> DiscoverAsync(CancellationToken cancellationToken = default);
    Task<HisenseVidaaDevice> ProbeAsync(string ipAddress, CancellationToken cancellationToken = default);
    Task<HisenseVidaaDevice> FindAsync(string ipAddress, CancellationToken cancellationToken = default);
    void SaveAddress(string monitorId, string ipAddress, string macAddress, string deviceUuid, string certificatePath);
    Task RequestPinAsync(string monitorId, CancellationToken cancellationToken = default);
    Task PairAsync(string monitorId, string pin, CancellationToken cancellationToken = default);
    void SaveKeyMacro(string monitorId, string sequence);
    Task SendKeyAsync(string monitorId, string key, CancellationToken cancellationToken = default);
    Task SetPictureSettingAsync(
        string monitorId,
        int menuId,
        int value,
        CancellationToken cancellationToken = default);
    Task<IReadOnlyList<VidaaPictureSetting>> GetPictureSettingsAsync(
        string monitorId,
        CancellationToken cancellationToken = default);
    Task<int> GetVolumeAsync(string monitorId, CancellationToken cancellationToken = default);
    Task<int> SetVolumeAsync(string monitorId, int volume, CancellationToken cancellationToken = default);
    Task SendPlatformActionAsync(
        string monitorId,
        string action,
        int value,
        CancellationToken cancellationToken = default);
    Task SendSequenceAsync(
        string monitorId,
        IEnumerable<(string Key, TimeSpan DelayAfter)> sequence,
        CancellationToken cancellationToken = default);
    Task<IReadOnlyList<VidaaTrafficMessage>> CaptureTrafficAsync(
        string monitorId,
        TimeSpan duration,
        CancellationToken cancellationToken = default);
    Task ListenTrafficAsync(
        string monitorId,
        Action<VidaaTrafficMessage> onMessage,
        CancellationToken cancellationToken = default);
    Task PowerOnAsync(string monitorId, CancellationToken cancellationToken = default);
}
