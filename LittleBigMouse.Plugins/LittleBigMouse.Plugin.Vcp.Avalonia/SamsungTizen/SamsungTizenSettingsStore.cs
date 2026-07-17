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
    Dictionary<string, SamsungTizenConfiguration>? _settings;

    public SamsungTizenSettingsStore()
        : this(Path.Combine(LbmPaths.ConfigDir, "samsung-tizen.json"))
    {
    }

    public SamsungTizenSettingsStore(string filePath) => _filePath = filePath;

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
            _settings = File.Exists(_filePath)
                ? JsonSerializer.Deserialize<Dictionary<string, SamsungTizenConfiguration>>(
                    File.ReadAllText(_filePath), JsonOptions) ?? []
                : [];
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
            File.WriteAllText(temporary, JsonSerializer.Serialize(_settings, JsonOptions));
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
        DeviceId = source.DeviceId,
        Name = source.Name,
        ModelName = source.ModelName,
        PictureMacro = source.PictureMacro,
    };
}
