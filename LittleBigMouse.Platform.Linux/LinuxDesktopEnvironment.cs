#nullable enable
using System;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Interprets the freedesktop environment variables that tell us which desktop shell
/// is running. Every KScreen/Plasma-specific code path used to inline the same
/// <c>XDG_CURRENT_DESKTOP.Contains("KDE")</c> check; this type centralises that so the
/// spelling of the check lives in one place and can be substituted in tests.
///
/// <para><c>XDG_CURRENT_DESKTOP</c> is, per the freedesktop spec, a colon-separated
/// list of names in priority order (e.g. <c>"ubuntu:GNOME"</c>). We treat the session as
/// KDE when any element names KDE or Plasma, case-insensitively. When that variable is
/// absent we fall back to <c>DESKTOP_SESSION</c>, which display managers set to the
/// session file name (e.g. <c>"plasma"</c>, <c>"plasmawayland"</c>).</para>
///
/// This is deliberately narrow: it is not a general cross-platform abstraction, only the
/// one signal the Linux platform code needs.
/// </summary>
public sealed class LinuxDesktopEnvironment
{
    readonly string? _xdgCurrentDesktop;
    readonly string? _desktopSession;

    /// <param name="xdgCurrentDesktop">Raw value of <c>XDG_CURRENT_DESKTOP</c> (may be null).</param>
    /// <param name="desktopSession">Raw value of <c>DESKTOP_SESSION</c> (may be null).</param>
    public LinuxDesktopEnvironment(string? xdgCurrentDesktop, string? desktopSession = null)
    {
        _xdgCurrentDesktop = xdgCurrentDesktop;
        _desktopSession = desktopSession;
    }

    /// <summary>The live process environment.</summary>
    public static LinuxDesktopEnvironment Current { get; } = new(
        Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP"),
        Environment.GetEnvironmentVariable("DESKTOP_SESSION"));

    /// <summary>
    /// True when the current session is KDE Plasma. Matches the historical behaviour
    /// (a "KDE" substring in <c>XDG_CURRENT_DESKTOP</c>) and additionally recognises the
    /// "plasma" spelling and the <c>DESKTOP_SESSION</c> fallback.
    /// </summary>
    public bool IsKde
        => string.IsNullOrEmpty(_xdgCurrentDesktop)
            ? NamesKde(_desktopSession)   // XDG unset: fall back to the session file name
            : NamesKde(_xdgCurrentDesktop);

    static bool NamesKde(string? value)
    {
        if (string.IsNullOrEmpty(value)) return false;

        // XDG_CURRENT_DESKTOP is a colon-separated priority list.
        foreach (var token in value.Split(':', StringSplitOptions.RemoveEmptyEntries))
        {
            if (token.Contains("KDE", StringComparison.OrdinalIgnoreCase)
                || token.Contains("plasma", StringComparison.OrdinalIgnoreCase))
                return true;
        }

        return false;
    }
}
