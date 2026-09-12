#nullable enable
namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: whether this desktop environment is one whose wallpaper can be set
/// per screen. Only the question is left here (v6): the agent is what sets it, so the
/// window asks this to decide whether the wallpaper editor is worth offering at all.
/// </summary>
public interface IWallpaperService
{
    bool IsSupported { get; }
}
