#nullable enable
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// Holds the VIDAA access and refresh tokens, encrypted — see
/// <see cref="EncryptedJsonStore{T}"/> for what that does and does not buy.
/// </summary>
public sealed class HisenseVidaaSettingsStore : EncryptedJsonStore<HisenseVidaaConfiguration>
{
    const string FileName = "hisense-vidaa.json";

    public HisenseVidaaSettingsStore()
        : this(Path.Combine(LbmPaths.ConfigDir, FileName), LegacyFilePath)
    {
    }

    public HisenseVidaaSettingsStore(string filePath) : this(filePath, null)
    {
    }

    /// <remarks>
    /// Internal on purpose: the public constructors have to stay shaped like
    /// <c>SamsungTizenSettingsStore</c>'s, since the container resolves both by picking the
    /// greediest constructor it can satisfy and neither takes a registered service.
    /// </remarks>
    internal HisenseVidaaSettingsStore(string filePath, string? legacyFilePath)
        : base("Hisense/VIDAA", filePath, legacyFilePath)
    {
    }

    /// <summary>
    /// Up to 5.6.0 this store ignored <see cref="LbmPaths"/> and built its own path, which put
    /// the file outside the configured configuration directory on Windows
    /// (<c>%APPDATA%</c> rather than <c>%LOCALAPPDATA%\Mgth</c>). On Unix the two resolve to
    /// the same <c>~/.config/LittleBigMouse</c> unless XDG_CONFIG_HOME says otherwise.
    /// </summary>
    static string LegacyFilePath => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LittleBigMouse", FileName);

    protected override HisenseVidaaConfiguration Clone(HisenseVidaaConfiguration c) => new() { MonitorId=c.MonitorId, IpAddress=c.IpAddress, MacAddress=c.MacAddress, ControllerMacAddress=c.ControllerMacAddress, DeviceUuid=c.DeviceUuid, ClientCertificatePath=c.ClientCertificatePath, ClientKeyPath=c.ClientKeyPath, ClientCertificatePassword=c.ClientCertificatePassword, Brand=c.Brand, ProtocolVersion=c.ProtocolVersion, AuthMethod=c.AuthMethod, ClientId=c.ClientId, MqttUsername=c.MqttUsername, AccessToken=c.AccessToken, RefreshToken=c.RefreshToken, AccessTokenIssuedAt=c.AccessTokenIssuedAt, RefreshTokenIssuedAt=c.RefreshTokenIssuedAt, AccessTokenDurationDays=c.AccessTokenDurationDays, RefreshTokenDurationDays=c.RefreshTokenDurationDays, LegacyAuthorized=c.LegacyAuthorized, KeyMacro=c.KeyMacro };
}
