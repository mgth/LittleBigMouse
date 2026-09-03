using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The store name of a layout (#589). Nine monitor ids join into a layout id longer than a
/// registry key name may be; the read threw and the UI never booted, which read as an
/// "8-monitor limit". A short id must keep its historical key; a long one must fit, stay
/// stable, and stay distinct from its neighbors.
/// </summary>
public class LayoutStoreKeyTests
{
    /// <summary>One Windows monitor id: PnP code, EDID serial, week, year, checksum — 29 chars.</summary>
    public static string MonitorId(int i) => $"GSM5B09{i:D2}NTMX5X567_0C_07E5_A{i % 10}";

    /// <summary>The layout id of that many monitors, as ComputeId joins them.</summary>
    public static string LayoutId(int monitors)
        => string.Join("+", Enumerable.Range(1, monitors).Select(MonitorId));

    [Fact]
    public void EightMonitors_KeepTheirHistoricalKey()
    {
        var id = LayoutId(8);

        Assert.True(id.Length <= LayoutStoreKey.MaxLength);
        Assert.Equal(id, LayoutStoreKey.For(id));
    }

    [Fact]
    public void NineMonitors_FitTheLimit()
    {
        // The reporter's configuration: the ninth monitor is the one that went over.
        var id = LayoutId(9);
        Assert.True(id.Length > LayoutStoreKey.MaxLength);

        Assert.True(LayoutStoreKey.For(id).Length <= LayoutStoreKey.MaxLength);
    }

    [Fact]
    public void LongId_KeepsAReadableHead()
        => Assert.StartsWith(MonitorId(1) + "+" + MonitorId(2), LayoutStoreKey.For(LayoutId(9)));

    [Fact]
    public void LongId_IsStable()
        => Assert.Equal(LayoutStoreKey.For(LayoutId(9)), LayoutStoreKey.For(LayoutId(9)));

    [Fact]
    public void LongIds_SharingTheirHead_GetDistinctKeys()
    {
        var a = LayoutId(9);
        // Differs in its last character only, far past the readable head.
        var b = a[..^1] + "F";

        Assert.NotEqual(LayoutStoreKey.For(a), LayoutStoreKey.For(b));
    }

    [Fact]
    public void Key_UsesOnlyStoreSafeCharacters()
        => Assert.Matches("^[A-Za-z0-9+_~]+$", LayoutStoreKey.For(LayoutId(12)));

    [Fact]
    public void ShorterCap_IsHonored()
    {
        // What the JSON store passes: the file name cap minus its extension.
        var key = LayoutStoreKey.For(LayoutId(9), 250);

        Assert.True(key.Length <= 250);
        Assert.NotEqual(LayoutStoreKey.For(LayoutId(9)), key);
    }

    [Fact]
    public void CapWithNoRoomForTheDigest_IsRefused()
        => Assert.Throws<ArgumentOutOfRangeException>(() => LayoutStoreKey.For(LayoutId(9), 40));
}
