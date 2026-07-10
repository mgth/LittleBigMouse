using System.Text.Json;
using System.Text.Json.Serialization;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

public enum WallpaperMode
{
    /// <summary>Each screen has its own image or color.</summary>
    PerScreen,
    /// <summary>One image over the whole monitor set, sliced in physical mm space.</summary>
    Span,
}

public enum ScreenWallpaperKind { Image, Color }

public class ScreenWallpaperSettings
{
    public ScreenWallpaperKind Kind { get; set; } = ScreenWallpaperKind.Image;
    public string? ImagePath { get; set; }
    [JsonConverter(typeof(JsonStringEnumConverter))]
    public WallpaperStyle Style { get; set; } = WallpaperStyle.Fill;
    /// <summary>#RRGGBB</summary>
    public string Color { get; set; } = "#204060";
}

public class LayoutWallpaperSettings
{
    [JsonConverter(typeof(JsonStringEnumConverter))]
    public WallpaperMode Mode { get; set; } = WallpaperMode.PerScreen;
    public string? SpanImagePath { get; set; }
    public Dictionary<string, ScreenWallpaperSettings> PerScreen { get; set; } = [];

    /// <summary>The user configured something worth pushing to the system.</summary>
    [JsonIgnore]
    public bool HasContent => Mode == WallpaperMode.Span
        ? !string.IsNullOrEmpty(SpanImagePath)
        : PerScreen.Values.Any(s => s.Kind == ScreenWallpaperKind.Color || !string.IsNullOrEmpty(s.ImagePath));
}

/// <summary>
/// Plugin-owned store, one entry per layout id — wallpapers are applied live,
/// outside the layout's Save/undo flow, so they stay out of the persisted
/// layout model (which would drag both OS persistence backends along).
/// </summary>
public static class WallpaperSettingsStore
{
    static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    public static string FilePath => Path.Combine(LbmPaths.ConfigDir, "wallpaper.json");

    public static Dictionary<string, LayoutWallpaperSettings> Load()
    {
        try
        {
            if (File.Exists(FilePath))
                return JsonSerializer.Deserialize<Dictionary<string, LayoutWallpaperSettings>>(
                    File.ReadAllText(FilePath), JsonOptions) ?? [];
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Wallpaper settings unreadable, starting fresh: {e.Message}");
        }
        return [];
    }

    public static void Save(Dictionary<string, LayoutWallpaperSettings> all)
    {
        try
        {
            Directory.CreateDirectory(LbmPaths.ConfigDir);
            var tmp = FilePath + ".tmp";
            File.WriteAllText(tmp, JsonSerializer.Serialize(all, JsonOptions));
            File.Move(tmp, FilePath, overwrite: true);
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Wallpaper settings not saved: {e.Message}");
        }
    }
}
