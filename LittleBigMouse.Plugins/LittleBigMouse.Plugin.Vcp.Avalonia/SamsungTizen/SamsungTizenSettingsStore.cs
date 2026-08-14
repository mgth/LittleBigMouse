#nullable enable
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

/// <summary>
/// The Samsung remote token only authorizes this application on the display's local network,
/// but it is a credential all the same, so it is stored encrypted — see
/// <see cref="EncryptedJsonStore{T}"/> for what that does and does not buy.
/// </summary>
public sealed class SamsungTizenSettingsStore : EncryptedJsonStore<SamsungTizenConfiguration>
{
    public SamsungTizenSettingsStore()
        : this(Path.Combine(LbmPaths.ConfigDir, "samsung-tizen.json"))
    {
    }

    public SamsungTizenSettingsStore(string filePath) : base("Samsung/Tizen", filePath)
    {
    }

    protected override SamsungTizenConfiguration Clone(SamsungTizenConfiguration source) => new()
    {
        MonitorId = source.MonitorId,
        IpAddress = source.IpAddress,
        MacAddress = source.MacAddress,
        Token = source.Token,
        DeviceId = source.DeviceId,
        Name = source.Name,
        ModelName = source.ModelName,
        PictureMacro = source.PictureMacro,
    };
}
