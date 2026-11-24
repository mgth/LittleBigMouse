using HLab.Geo;
using HLab.Sys.Windows.API;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Windows;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The pairing between the order the shell files per-monitor wallpapers in and the order it
/// paints them in (<see cref="WallpaperSlotOrder"/>), on the preview read-back
/// (<see cref="WindowsWallpaperMapping.RebindPaintedImages(WallpaperSlotOrder.SlotPlan, IEnumerable{DisplaySource}, string?)"/>).
/// The writer is the agent's in v6, tested in Rust (<c>lbm_wallpaper::slots</c>).
/// <para>
/// Everything runs against <see cref="FakeDesktop"/>, a scripted <see cref="IDesktopWallpaper"/>,
/// so it runs off Windows. What it encodes about the shell — slot <c>k</c> is painted on the
/// k-th monitor of "primary first, then filing order" — is the contributor's hardware finding
/// from the two-monitor case, not something these tests prove.
/// </para>
/// </summary>
public class WallpaperSlotOrderTests
{
    // Primary at the origin, a portrait screen at its left, a landscape one at its right.
    static readonly WinDef.Rect Primary = new(0, 0, 1920, 1080);
    static readonly WinDef.Rect Left = new(-1080, -420, 0, 1500);
    static readonly WinDef.Rect Right = new(1920, 0, 3840, 1080);

    const string PathP = @"\\?\DISPLAY#PRI#1";
    const string PathL = @"\\?\DISPLAY#LEFT#1";
    const string PathR = @"\\?\DISPLAY#RIGHT#1";


    // ---- Preview read-back -------------------------------------------------------

    [Fact]
    public void Preview_PhotoAndSolidColor_PaintsThePhotoOnce()
    {
        // On screen: Left shows the photo, the primary a solid colour. The Left path files the
        // primary's slot (empty), the primary path files Left's (the photo). The device-path
        // read-back and the registry both say "photo" for the two of them.
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary))
        {
            Images = { [PathL] = "", [PathP] = @"C:\photo.png" },
        };
        var left = Source("L", Left, @"C:\photo.png");
        var primary = Source("P", Primary, @"C:\photo.png");

        WindowsWallpaperMapping.RebindPaintedImages(PlanOf(desktop, 2), [left, primary], @"C:\photo.png");

        Assert.Equal(@"C:\photo.png", left.WallpaperPath);
        Assert.Equal("", primary.WallpaperPath);
    }

    [Fact]
    public void Preview_FadeTransient_KeepsTheRegistryFallback()
    {
        // During the fade every slot reads "": the registry holds the incoming image.
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary))
        {
            Images = { [PathL] = "", [PathP] = "" },
        };
        var left = Source("L", Left, @"C:\new.png");
        var primary = Source("P", Primary, @"C:\new.png");

        WindowsWallpaperMapping.RebindPaintedImages(PlanOf(desktop, 2), [left, primary], @"C:\new.png");

        Assert.Equal(@"C:\new.png", left.WallpaperPath);
        Assert.Equal(@"C:\new.png", primary.WallpaperPath);
    }

    [Fact]
    public void Preview_SwappedImages_FollowThePaintOrder()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary))
        {
            Images = { [PathL] = @"C:\a.png", [PathP] = @"C:\b.png" },
        };
        var left = Source("L", Left, @"C:\a.png");
        var primary = Source("P", Primary, @"C:\b.png");

        WindowsWallpaperMapping.RebindPaintedImages(PlanOf(desktop, 2), [left, primary], @"C:\b.png");

        Assert.Equal(@"C:\b.png", left.WallpaperPath);
        Assert.Equal(@"C:\a.png", primary.WallpaperPath);
    }

    [Fact]
    public void Preview_UnreadableImage_KeepsTheDevicePathReadBack()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary))
        {
            Images = { [PathL] = @"C:\a.png", [PathP] = @"C:\b.png" },
            ImageReadFailures = { PathL },
        };
        var left = Source("L", Left, @"C:\a.png");
        var primary = Source("P", Primary, @"C:\b.png");

        WindowsWallpaperMapping.RebindPaintedImages(PlanOf(desktop, 2), [left, primary], "");

        Assert.Equal(@"C:\b.png", left.WallpaperPath);
        // The Left slot paints the primary but could not be read: the primary is left as the
        // device-path read-back gave it, rather than blanked as a solid colour.
        Assert.Equal(@"C:\b.png", primary.WallpaperPath);
    }

    // ---- Plan ----------------------------------------------------------------------

    [Fact]
    public void Plan_PrimaryFirst_IsIdentity()
    {
        var plan = PlanOf(new FakeDesktop((PathP, Primary), (PathL, Left)), 2);

        Assert.All(plan.Pairs, p => Assert.Equal(p.Filed.DevicePath, p.Painted.DevicePath));
        Assert.True(plan.Complete);
    }

    [Fact]
    public void Plan_DetachedEntryBesideAFailedLookup_KeepsTheCertainPairs()
    {
        // Three active monitors (Left, Right, Primary) and a historical one. The historical
        // one answers S_FALSE (not attached), Right fails: Right is the hole, not the
        // historical entry, and the Left slot is still known to paint the primary.
        var desktop = new FakeDesktop((PathL, Left), (@"\\?\DISPLAY#OLD#1", new WinDef.Rect()), (PathR, Right), (PathP, Primary))
        {
            Detached = { @"\\?\DISPLAY#OLD#1" },
            RectFailures = { [PathR] = int.MaxValue },
        };

        var plan = PlanOf(desktop, 3);

        Assert.False(plan.Complete);
        Assert.Equal([(PathL, PathP)], plan.Pairs.Select(p => (p.Filed.DevicePath, p.Painted.DevicePath)));
    }

    [Fact]
    public void Plan_CountThatDoesNotAddUp_IsUnusable()
    {
        // Two unreadable slots, one missing monitor: which one is on the desktop is unknown.
        var reads = new List<WallpaperSlotOrder.SlotRead>
        {
            new(PathP, Primary, ""),
            new(PathL, null, null),
            new(PathR, null, null),
        };

        Assert.Null(WallpaperSlotOrder.Plan(reads, 2));
    }

    [Fact]
    public void Plan_UnreadablePrimary_IsPlacedWhenItIsTheOnlyHole()
    {
        var reads = new List<WallpaperSlotOrder.SlotRead>
        {
            new(PathL, Left, "l"),
            new(PathP, null, null),
            new(PathR, Right, "r"),
        };

        var plan = WallpaperSlotOrder.Plan(reads, 3)!;

        // Filing L, P, R → painting P, L, R: only the R slot pairs two readable monitors.
        Assert.False(plan.Complete);
        Assert.Equal([(PathR, PathR)], plan.Pairs.Select(p => (p.Filed.DevicePath, p.Painted.DevicePath)));
    }

    // ---- Helpers -------------------------------------------------------------------

    static WallpaperSlotOrder.SlotPlan PlanOf(FakeDesktop desktop, int active)
        => WallpaperSlotOrder.Plan(WallpaperSlotOrder.Read(desktop)!, active)!;

    static DisplaySource Source(string id, WinDef.Rect rect, string wallpaper)
    {
        var source = new DisplaySource(id) { WallpaperPath = wallpaper };
        source.InPixel.Set(new Rect(rect.X, rect.Y, rect.Width, rect.Height));
        return source;
    }

    /// <summary>A scripted shell: slots in filing order, with injectable failures.</summary>
    sealed class FakeDesktop(params (string Path, WinDef.Rect Rect)[] slots) : IDesktopWallpaper
    {
        public Dictionary<string, string> Images { get; } = [];
        public List<(string Path, string Image)> Writes { get; } = [];
        public HashSet<string> RejectWritesTo { get; } = [];
        public HashSet<string> ImageReadFailures { get; } = [];
        /// <summary>Per path, how many GetMonitorRECT calls fail before it succeeds.</summary>
        public Dictionary<string, int> RectFailures { get; } = [];
        public bool CountFails { get; init; }
        /// <summary>Paths whose GetMonitorRECT answers S_FALSE: listed, not attached.</summary>
        public HashSet<string> Detached { get; } = [];

        public HResult SetWallpaper(string monitorId, string wallpaper)
        {
            Writes.Add((monitorId, wallpaper));
            if (RejectWritesTo.Contains(monitorId)) return HResult.Fail;
            Images[monitorId] = wallpaper;
            return HResult.Ok;
        }

        public HResult GetWallpaper(string monitorId, ref string wallpaper)
        {
            if (ImageReadFailures.Contains(monitorId)) return HResult.Fail;
            wallpaper = Images.GetValueOrDefault(monitorId, "");
            return HResult.Ok;
        }

        public HResult GetMonitorDevicePathAt(uint monitorIndex, ref string monitorId)
        {
            monitorId = slots[monitorIndex].Path;
            return HResult.Ok;
        }

        public HResult GetMonitorDevicePathCount(ref uint count)
        {
            if (CountFails) return HResult.Fail;
            count = (uint)slots.Length;
            return HResult.Ok;
        }

        public HResult GetMonitorRECT(string monitorId, ref WinDef.Rect displayRect)
        {
            if (Detached.Contains(monitorId)) return HResult.False;
            if (RectFailures.TryGetValue(monitorId, out var left) && left > 0)
            {
                RectFailures[monitorId] = left - 1;
                return HResult.Fail;
            }
            displayRect = slots.First(s => s.Path == monitorId).Rect;
            return HResult.Ok;
        }

        public HResult SetBackgroundColor(uint color) => HResult.Ok;
        public HResult GetBackgroundColor(ref uint color) => HResult.Ok;
        public HResult SetPosition(DesktopWallpaperPosition position) => HResult.Ok;
        public HResult GetPosition(ref DesktopWallpaperPosition position) => HResult.Ok;
        public HResult SetSlideshow(IShellItemArray items) => throw new NotSupportedException();
        public HResult GetSlideshow(ref IShellItemArray items) => throw new NotSupportedException();
        public HResult SetSlideshowOptions(DesktopSlideshowOptions options, uint slideshowTick) => throw new NotSupportedException();
        public HResult GetSlideshowOptions(out DesktopSlideshowOptions options, out uint slideshowTick) => throw new NotSupportedException();
        public HResult AdvanceSlideshow(string monitorId, DesktopSlideshowDirection direction) => throw new NotSupportedException();
        public HResult GetStatus(ref DesktopSlideshowState state) => throw new NotSupportedException();
        public HResult Enable(bool enable) => throw new NotSupportedException();
    }
}
