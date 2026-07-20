#nullable enable
using System.Text.Json;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

/// <summary>
/// Atomic, plugin-owned storage. The remote token only authorizes this application on
/// the display's local network; on Unix the file is nevertheless restricted to its owner.
/// </summary>
public sealed class SamsungTizenSettingsStore
{
    static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };
    readonly object _lock = new();
    readonly string _filePath;
    readonly SmartTvSecretProtector _secrets;
    Dictionary<string, SamsungTizenConfiguration>? _settings;

    public SamsungTizenSettingsStore()
        : this(Path.Combine(LbmPaths.ConfigDir, "samsung-tizen.json"))
    {
    }

    public SamsungTizenSettingsStore(string filePath)
    {
        _filePath = filePath;
        _secrets = new SmartTvSecretProtector(
            Path.GetDirectoryName(filePath) ?? LbmPaths.ConfigDir);
    }

    public SamsungTizenConfiguration? Get(string monitorId)
    {
        lock (_lock)
        {
            EnsureLoaded();
            return _settings!.TryGetValue(monitorId, out var value) ? Clone(value) : null;
        }
    }

    public void Save(SamsungTizenConfiguration configuration)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(configuration.MonitorId);

        lock (_lock)
        {
            EnsureLoaded();
            _settings![configuration.MonitorId] = Clone(configuration);
            SaveLocked();
        }
    }

    void EnsureLoaded()
    {
        if (_settings is not null) return;

        try
        {
            var stored = File.Exists(_filePath)
                ? JsonSerializer.Deserialize<Dictionary<string, SamsungTizenConfiguration>>(
                    File.ReadAllText(_filePath), JsonOptions) ?? []
                : [];
            _settings = stored.ToDictionary(item => item.Key,
                item => FromStored(item.Value));
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Samsung/Tizen settings unreadable, starting fresh: {e.Message}");
            _settings = [];
        }
    }

    void SaveLocked()
    {
        try
        {
            var directory = Path.GetDirectoryName(_filePath);
            if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

            var temporary = _filePath + ".tmp";
            var stored = _settings!.ToDictionary(item => item.Key,
                item => ToStored(item.Value));
            File.WriteAllText(temporary, JsonSerializer.Serialize(stored, JsonOptions));
            File.Move(temporary, _filePath, overwrite: true);

            if (!OperatingSystem.IsWindows())
                File.SetUnixFileMode(_filePath, UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Samsung/Tizen settings not saved: {e.Message}");
        }
    }

    static SamsungTizenConfiguration Clone(SamsungTizenConfiguration source) => new()
    {
        MonitorId = source.MonitorId,
        IpAddress = source.IpAddress,
        MacAddress = source.MacAddress,
        Token = source.Token,
        ServerCertificateFingerprint = source.ServerCertificateFingerprint,
        DeviceId = source.DeviceId,
        Name = source.Name,
        ModelName = source.ModelName,
        PictureMacro = source.PictureMacro,
    };

    SamsungTizenConfiguration ToStored(SamsungTizenConfiguration source)
    {
        var stored = Clone(source);
        stored.Token = _secrets.Protect(source.Token);
        return stored;
    }

    SamsungTizenConfiguration FromStored(SamsungTizenConfiguration source)
    {
        var value = Clone(source);
        try { value.Token = _secrets.Unprotect(source.Token); }
        catch (Exception error)
        {
            Console.Error.WriteLine($"Samsung/Tizen token could not be unlocked: {error.Message}");
            value.Token = "";
        }
        return value;
    }
}
