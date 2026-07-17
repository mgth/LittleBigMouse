#nullable enable
using System.Text.Json;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseVidaaSettingsStore
{
    readonly string _path;
    Dictionary<string, HisenseVidaaConfiguration>? _items;

    public HisenseVidaaSettingsStore()
    {
        var dir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LittleBigMouse");
        Directory.CreateDirectory(dir);
        _path = Path.Combine(dir, "hisense-vidaa.json");
    }
    public HisenseVidaaConfiguration? Get(string id)
    {
        Load();
        return _items!.TryGetValue(id, out var c) ? Clone(c) : null;
    }
    public void Save(HisenseVidaaConfiguration c)
    {
        Load(); _items![c.MonitorId] = Clone(c);
        File.WriteAllText(_path, JsonSerializer.Serialize(_items, new JsonSerializerOptions { WriteIndented = true }));
    }
    void Load()
    {
        if (_items is not null) return;
        try { _items = File.Exists(_path) ? JsonSerializer.Deserialize<Dictionary<string, HisenseVidaaConfiguration>>(File.ReadAllText(_path)) : []; }
        catch { _items = []; }
    }
    static HisenseVidaaConfiguration Clone(HisenseVidaaConfiguration c) => new() { MonitorId=c.MonitorId, IpAddress=c.IpAddress, MacAddress=c.MacAddress, ControllerMacAddress=c.ControllerMacAddress, DeviceUuid=c.DeviceUuid, ClientCertificatePath=c.ClientCertificatePath, ClientKeyPath=c.ClientKeyPath, ClientCertificatePassword=c.ClientCertificatePassword, Brand=c.Brand, ProtocolVersion=c.ProtocolVersion, AuthMethod=c.AuthMethod, ClientId=c.ClientId, MqttUsername=c.MqttUsername, AccessToken=c.AccessToken, RefreshToken=c.RefreshToken, AccessTokenIssuedAt=c.AccessTokenIssuedAt, RefreshTokenIssuedAt=c.RefreshTokenIssuedAt, AccessTokenDurationDays=c.AccessTokenDurationDays, RefreshTokenDurationDays=c.RefreshTokenDurationDays, LegacyAuthorized=c.LegacyAuthorized, KeyMacro=c.KeyMacro };
}
