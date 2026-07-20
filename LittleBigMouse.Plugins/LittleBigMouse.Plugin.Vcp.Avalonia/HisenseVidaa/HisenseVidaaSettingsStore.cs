#nullable enable
using System.Text.Json;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseVidaaSettingsStore
{
    static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };
    readonly object _lock = new();
    readonly string _path;
    readonly SmartTvSecretProtector _secrets;
    Dictionary<string, HisenseVidaaConfiguration>? _items;

    public HisenseVidaaSettingsStore()
        : this(Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "LittleBigMouse", "hisense-vidaa.json")) { }

    public HisenseVidaaSettingsStore(string path)
    {
        _path = path;
        _secrets = new SmartTvSecretProtector(
            Path.GetDirectoryName(path) ?? LbmPaths.ConfigDir);
    }

    public HisenseVidaaConfiguration? Get(string id)
    {
        lock (_lock)
        {
            Load();
            return _items!.TryGetValue(id, out var value) ? Clone(value) : null;
        }
    }

    public void Save(HisenseVidaaConfiguration configuration)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(configuration.MonitorId);
        lock (_lock)
        {
            Load();
            _items![configuration.MonitorId] = Clone(configuration);
            var stored = _items.ToDictionary(item => item.Key,
                item => ToStored(item.Value));
            var directory = Path.GetDirectoryName(_path);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);
            var temporary = _path + ".tmp";
            File.WriteAllText(temporary, JsonSerializer.Serialize(stored, JsonOptions));
            File.Move(temporary, _path, overwrite: true);
            if (!OperatingSystem.IsWindows())
                File.SetUnixFileMode(_path, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }

    void Load()
    {
        if (_items is not null) return;
        try
        {
            var stored = File.Exists(_path)
                ? JsonSerializer.Deserialize<Dictionary<string, HisenseVidaaConfiguration>>(
                    File.ReadAllText(_path), JsonOptions) ?? []
                : [];
            _items = stored.ToDictionary(item => item.Key,
                item => FromStored(item.Value));
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Hisense/VIDAA settings unreadable, starting fresh: {error.Message}");
            _items = [];
        }
    }

    HisenseVidaaConfiguration ToStored(HisenseVidaaConfiguration source)
    {
        var value = Clone(source);
        value.ClientCertificatePassword = _secrets.Protect(source.ClientCertificatePassword);
        value.ClientId = _secrets.Protect(source.ClientId);
        value.MqttUsername = _secrets.Protect(source.MqttUsername);
        value.AccessToken = _secrets.Protect(source.AccessToken);
        value.RefreshToken = _secrets.Protect(source.RefreshToken);
        return value;
    }

    HisenseVidaaConfiguration FromStored(HisenseVidaaConfiguration source)
    {
        var value = Clone(source);
        try
        {
            value.ClientCertificatePassword = _secrets.Unprotect(source.ClientCertificatePassword);
            value.ClientId = _secrets.Unprotect(source.ClientId);
            value.MqttUsername = _secrets.Unprotect(source.MqttUsername);
            value.AccessToken = _secrets.Unprotect(source.AccessToken);
            value.RefreshToken = _secrets.Unprotect(source.RefreshToken);
        }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Hisense/VIDAA credentials could not be unlocked: {error.Message}");
            value.ClientCertificatePassword = "";
            value.ClientId = "";
            value.MqttUsername = "";
            value.AccessToken = "";
            value.RefreshToken = "";
        }
        return value;
    }

    static HisenseVidaaConfiguration Clone(HisenseVidaaConfiguration c) => new()
    {
        MonitorId = c.MonitorId,
        IpAddress = c.IpAddress,
        MacAddress = c.MacAddress,
        ControllerMacAddress = c.ControllerMacAddress,
        DeviceUuid = c.DeviceUuid,
        ClientCertificatePath = c.ClientCertificatePath,
        ClientKeyPath = c.ClientKeyPath,
        ClientCertificatePassword = c.ClientCertificatePassword,
        ServerCertificateFingerprint = c.ServerCertificateFingerprint,
        Brand = c.Brand,
        ProtocolVersion = c.ProtocolVersion,
        AuthMethod = c.AuthMethod,
        ClientId = c.ClientId,
        MqttUsername = c.MqttUsername,
        AccessToken = c.AccessToken,
        RefreshToken = c.RefreshToken,
        AccessTokenIssuedAt = c.AccessTokenIssuedAt,
        RefreshTokenIssuedAt = c.RefreshTokenIssuedAt,
        AccessTokenDurationDays = c.AccessTokenDurationDays,
        RefreshTokenDurationDays = c.RefreshTokenDurationDays,
        LegacyAuthorized = c.LegacyAuthorized,
        KeyMacro = c.KeyMacro,
    };
}
