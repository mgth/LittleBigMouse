using System;
using System.Linq;
using System.Runtime.InteropServices;
using Avalonia.Data.Converters;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// Shows a shortcut the way the keyboard shows it.
/// <para>
/// A shortcut is stored, and registered, as a <em>position</em>: `Oem7` is the same
/// physical key whatever the layout prints on it, which is what `RegisterHotKey` takes
/// and what makes a recorded shortcut survive a layout change. Honest, and unreadable —
/// nobody pressed "Oem7", they pressed <c>²</c>.
/// </para>
/// <para>
/// So the position is what travels and the character is what is shown. Only the
/// display changes: the stored value, the wire and the daemon are untouched, and a
/// layout switch changes what is displayed without invalidating anything.
/// </para>
/// </summary>
public static class ShortcutDisplay
{
    public static readonly FuncValueConverter<string?, string> Readable = new(Readably);

    /// <summary>`Ctrl+Oem7` reads as `Ctrl+²` on an AZERTY board, `Ctrl+'` on a US one.</summary>
    internal static string Readably(string? shortcut)
    {
        if (string.IsNullOrWhiteSpace(shortcut)) return "";

        return string.Join("+", shortcut.Split('+').Select(part =>
        {
            var name = part.Trim();
            // Only the position-named keys need translating. Everything else already
            // reads as what it is — "M", "F12", "Space", "NumPad0".
            if (!name.StartsWith("Oem", StringComparison.Ordinal)) return name;
            return CharacterOf(name) ?? name;
        }));
    }

    /// <summary>
    /// What this key prints on the layout in force right now, or null when it prints
    /// nothing — a key with no character is better named than shown blank.
    /// </summary>
    static string? CharacterOf(string name)
    {
        if (!OperatingSystem.IsWindows()) return null;
        if (ShortcutBox.VirtualKey(name) is not { } vk) return null;

        try
        {
            // MAPVK_VK_TO_CHAR: the unshifted character, or 0 for none. The top bit
            // flags a dead key, which still has a character worth showing.
            var mapped = MapVirtualKeyW(vk, 2) & 0x7FFF;
            if (mapped == 0) return null;

            var c = (char)mapped;
            return char.IsControl(c) || char.IsWhiteSpace(c) ? null : c.ToString();
        }
        catch (DllNotFoundException)
        {
            return null;
        }
        catch (EntryPointNotFoundException)
        {
            return null;
        }
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern uint MapVirtualKeyW(uint uCode, uint uMapType);
}
