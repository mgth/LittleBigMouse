#nullable enable
using System.Threading.Tasks;

namespace LittleBigMouse.Plugins;

/// <summary>
/// The one thing the wallpaper editor asks of the agent: here are the settings for this
/// layout. The agent records them — it owns <c>wallpaper.json</c> — and paints the
/// desktop.
/// <para>
/// A seam rather than the client itself because the plugin assembly does not see the
/// window's, and because a frontend that cannot reach an agent must still be able to
/// show the editor.
/// </para>
/// </summary>
public interface IAgentWallpaper
{
    Task SaveWallpaperAsync(string layoutId, object settings);
}
