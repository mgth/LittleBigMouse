#nullable enable
using Avalonia.Input;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

/// <summary>Maps desktop keys to Samsung remote keys while the remote surface owns focus.</summary>
public static class SamsungRemoteKeyboard
{
    public static string? RemoteKeyFor(Key key)
        => RemoteKeyboardProfile.Samsung.RemoteKeyFor(key);
}
