using LittleBigMouse.Platform.Linux;
using Xunit;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The KScreen/Plasma code paths gate on <see cref="LinuxDesktopEnvironment.IsKde"/>.
/// These pin the interpretation of the freedesktop variables: colon-separated priority
/// lists, case-insensitivity, the "plasma" spelling, and the absent case that non-KDE
/// sessions rely on to fall back to xrandr / skip the wallpaper and gap code.
/// </summary>
public class LinuxDesktopEnvironmentTests
{
    [Theory]
    // Absent / empty: not KDE (non-KDE sessions must keep their xrandr fallback).
    [InlineData(null, false)]
    [InlineData("", false)]
    // Canonical KDE value, as reported by a Plasma session.
    [InlineData("KDE", true)]
    // The "plasma" spelling some setups use.
    [InlineData("plasma", true)]
    // Case variations.
    [InlineData("kde", true)]
    [InlineData("Kde", true)]
    [InlineData("Plasma", true)]
    [InlineData("PLASMA", true)]
    // Colon-separated priority list: KDE present anywhere in the list.
    [InlineData("ubuntu:KDE", true)]
    [InlineData("KDE:X-Generic", true)]
    [InlineData("foo:plasma:bar", true)]
    // Non-KDE desktops stay non-KDE.
    [InlineData("GNOME", false)]
    [InlineData("ubuntu:GNOME", false)]
    [InlineData("XFCE", false)]
    [InlineData("sway", false)]
    public void IsKde_from_xdg_current_desktop(string? xdgCurrentDesktop, bool expected)
    {
        var env = new LinuxDesktopEnvironment(xdgCurrentDesktop);
        Assert.Equal(expected, env.IsKde);
    }

    [Fact]
    public void DesktopSession_is_used_as_fallback_when_xdg_absent()
    {
        Assert.True(new LinuxDesktopEnvironment(null, "plasmawayland").IsKde);
        Assert.True(new LinuxDesktopEnvironment(null, "plasma").IsKde);
        Assert.False(new LinuxDesktopEnvironment(null, "gnome").IsKde);
        Assert.False(new LinuxDesktopEnvironment(null, null).IsKde);
    }

    [Fact]
    public void XdgCurrentDesktop_is_authoritative_when_present()
    {
        // A KDE XDG value stays KDE regardless of DESKTOP_SESSION.
        Assert.True(new LinuxDesktopEnvironment("KDE", "gnome").IsKde);
        // The DESKTOP_SESSION fallback only fires when XDG is absent: a present
        // non-KDE XDG value is authoritative and is NOT overridden by the session.
        Assert.False(new LinuxDesktopEnvironment("GNOME", "plasma").IsKde);
        // An empty XDG (not just null) also falls through to the session.
        Assert.True(new LinuxDesktopEnvironment("", "plasma").IsKde);
    }

    [Fact]
    public void Current_reads_the_live_environment_without_throwing()
    {
        // Smoke test: the static accessor must be usable from the platform code.
        _ = LinuxDesktopEnvironment.Current.IsKde;
    }
}
