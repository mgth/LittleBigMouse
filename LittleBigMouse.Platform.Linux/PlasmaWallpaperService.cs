#nullable enable
using System;
using System.Collections.Generic;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Writes per-screen wallpapers through the same plasmashell scripting API
/// PlasmaWallpaper reads from: screens matched by logical geometry, org.kde.image
/// for pictures, org.kde.color for solid colors, one script per batch so plasma
/// rewrites appletsrc once.
/// </summary>
public class PlasmaWallpaperService : IWallpaperService
{
    bool? _isSupported;

    public bool IsSupported => _isSupported ??=
        Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP")?.Contains("KDE", StringComparison.OrdinalIgnoreCase) == true
        && PlasmaWallpaper.EvaluateScript("print(\"ok\")") == "ok";

    public Task ApplyAsync(IReadOnlyList<ScreenWallpaper> screens)
        => Task.Run(() =>
        {
            if (screens.Count == 0) return;
            PlasmaWallpaper.EvaluateScript(BuildScript(screens));
        });

    /// <summary>org.kde.image FillMode is a QML Image.fillMode value (inverse of the read mapping).</summary>
    static int FillMode(WallpaperStyle style) => style switch
    {
        WallpaperStyle.Stretch => 0,
        WallpaperStyle.Fit => 1,
        WallpaperStyle.Tile => 3,
        WallpaperStyle.Center => 6,
        _ => 2, // Fill (PreserveAspectCrop), Span is per-screen meaningless here
    };

    static string BuildScript(IReadOnlyList<ScreenWallpaper> screens)
    {
        var targets = new List<object>();
        foreach (var screen in screens)
        {
            if (screen.ImagePath is { Length: > 0 } path)
                targets.Add(new
                {
                    x = screen.LogicalBounds.X,
                    y = screen.LogicalBounds.Y,
                    image = new Uri(path).AbsoluteUri,
                    fill = FillMode(screen.Style),
                });
            else if (screen.Color is { } color)
            {
                var rgb = color.To<byte>();
                targets.Add(new
                {
                    x = screen.LogicalBounds.X,
                    y = screen.LogicalBounds.Y,
                    color = $"{rgb.Red},{rgb.Green},{rgb.Blue}",
                });
            }
        }

        var script = new StringBuilder();
        script.Append("var targets=").Append(JsonSerializer.Serialize(targets)).Append(';');
        script.Append(
            "for(var i=0;i<screenCount;i++){" +
            "var g=screenGeometry(i);" +
            "for(var j=0;j<targets.length;j++){var t=targets[j];" +
            "if(Math.abs(g.x-t.x)<2&&Math.abs(g.y-t.y)<2){" +
            "var d=desktopForScreen(i);" +
            "if(t.image){" +
            "d.wallpaperPlugin=\"org.kde.image\";" +
            "d.currentConfigGroup=[\"Wallpaper\",\"org.kde.image\",\"General\"];" +
            "d.writeConfig(\"Image\",t.image);" +
            "d.writeConfig(\"FillMode\",t.fill);" +
            "}else{" +
            "d.wallpaperPlugin=\"org.kde.color\";" +
            "d.currentConfigGroup=[\"Wallpaper\",\"org.kde.color\",\"General\"];" +
            "d.writeConfig(\"Color\",t.color);" +
            "}" +
            "d.reloadConfig();" +
            "}}}" +
            "print(\"ok\");");
        return script.ToString();
    }
}
