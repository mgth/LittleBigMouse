#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using System.Text.Json.Serialization;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux <see cref="ILayoutStore"/>: JSON files under ~/.config/LittleBigMouse (XDG).
/// The structure mirrors the Windows registry tree so field semantics stay 1:1:
/// <c>options.json</c> = the root key, <c>models.json</c> = the per-PnP "monitors" keys,
/// <c>layouts/&lt;id&gt;.json</c> = one "Layouts\{id}" key with its monitors and sources.
/// Writes are atomic (temp file + rename). The config dir is injectable for tests.
/// </summary>
public class JsonLayoutStore : ILayoutStore
{
    static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull
    };

    readonly string _configDir;

    public JsonLayoutStore() : this(LbmPaths.ConfigDir) { }

    public JsonLayoutStore(string configDir) => _configDir = configDir;

    string OptionsPath => Path.Combine(_configDir, "options.json");
    string ModelsPath => Path.Combine(_configDir, "models.json");

    string LayoutPath(string layoutId)
    {
        var id = string.Join("_", layoutId.Split(Path.GetInvalidFileNameChars(), StringSplitOptions.RemoveEmptyEntries));
        return Path.Combine(_configDir, "layouts", $"{id}.json");
    }

    public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes) => new()
    {
        GlobalOptions = ReadJson<GlobalOptionsDto>(OptionsPath),
        Layout = ReadJson<LayoutDto>(LayoutPath(layoutId)),
        Models = ReadJson<Dictionary<string, ModelDto>>(ModelsPath) ?? []
    };

    public void WriteGlobalOptions(GlobalOptionsDto options) => WriteJson(OptionsPath, options);

    public void WriteLayout(string layoutId, LayoutDto layout) => WriteJson(LayoutPath(layoutId), layout);

    public void WriteModels(IReadOnlyDictionary<string, ModelDto> models)
    {
        var all = ReadJson<Dictionary<string, ModelDto>>(ModelsPath) ?? [];
        foreach (var (pnpCode, model) in models) all[pnpCode] = model;
        WriteJson(ModelsPath, all);
    }

    static T? ReadJson<T>(string path) where T : class
    {
        try
        {
            if (!File.Exists(path)) return null;
            return JsonSerializer.Deserialize<T>(File.ReadAllText(path), JsonOptions);
        }
        catch
        {
            // A corrupt file must never prevent the app from starting: fall back to defaults.
            return null;
        }
    }

    static void WriteJson<T>(string path, T value)
    {
        try
        {
            Directory.CreateDirectory(Path.GetDirectoryName(path)!);
            var temp = path + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(value, JsonOptions));
            File.Move(temp, path, overwrite: true);
        }
        catch { }
    }
}
