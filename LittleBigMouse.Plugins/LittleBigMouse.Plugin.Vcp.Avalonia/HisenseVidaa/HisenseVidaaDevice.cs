#nullable enable
namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed record HisenseVidaaDevice(string IpAddress, string Name = "Hisense VIDAA", string ModelName = "", string MacAddress = "", int? ProtocolVersion = null, string Brand = "his")
{
    public static bool IsValidAddress(string value) => System.Net.IPAddress.TryParse(value?.Trim(), out var ip) && ip.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork;
}

public sealed class HisenseVidaaConfiguration
{
    public string MonitorId { get; set; } = "";
    public string IpAddress { get; set; } = "";
    public string MacAddress { get; set; } = "";
    public string ControllerMacAddress { get; set; } = "";
    public string DeviceUuid { get; set; } = "";
    public string ClientCertificatePath { get; set; } = "";
    public string ClientKeyPath { get; set; } = "";
    public string ClientCertificatePassword { get; set; } = VidaaCertificate.DefaultPassword;
    public string ServerCertificateFingerprint { get; set; } = "";
    public string Brand { get; set; } = "his";
    public int? ProtocolVersion { get; set; }
    public VidaaAuthMethod AuthMethod { get; set; }
    public string ClientId { get; set; } = "";
    public string MqttUsername { get; set; } = "";
    public string AccessToken { get; set; } = "";
    public string RefreshToken { get; set; } = "";
    public DateTimeOffset AccessTokenIssuedAt { get; set; }
    public DateTimeOffset RefreshTokenIssuedAt { get; set; }
    public int AccessTokenDurationDays { get; set; }
    public int RefreshTokenDurationDays { get; set; }
    public bool LegacyAuthorized { get; set; }
    public string KeyMacro { get; set; } = "KEY_BRIGHTNESSUP";
    public bool HasPairing => !string.IsNullOrWhiteSpace(ServerCertificateFingerprint)
        && (LegacyAuthorized
            || (!string.IsNullOrWhiteSpace(ClientId) && !string.IsNullOrWhiteSpace(AccessToken)));
}
