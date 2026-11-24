using HLab.Geo;
using HLab.Sys.Windows.API;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The pairing between the order the shell files per-monitor wallpapers in and the order it
/// paints them in (<see cref="WallpaperSlotOrder"/>), on both ends: the writer
/// (<see cref="WindowsWallpaperService.WriteImages"/>) and the preview read-back
/// (<see cref="WindowsWallpaperMapping.RebindPaintedImages(WallpaperSlotOrder.SlotPlan, IEnumerable{DisplaySource}, string?)"/>).
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

    static readonly TimeSpan NoDelay = TimeSpan.Zero;

    // ---- Writer ------------------------------------------------------------------

    [Fact]
    public void Write_PrimaryNotFirst_FilesEachImageUnderTheSlotPaintedOnItsScreen()
    {
        // Filing order Left, Primary; painting order Primary, Left.
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary));

        var applied = WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Primary, "primary.png", PathP)], 2, NoDelay);

        Assert.True(applied);
        Assert.Equal("primary.png", desktop.Images[PathL]);
        Assert.Equal("left.png", desktop.Images[PathP]);
    }

    [Fact]
    public void Write_OneFailedWriteInASwappedPair_DoesNotOverwriteTheSuccessfulOne()
    {
        // The primary's image goes in through the left path and succeeds; the left image cannot
        // be filed under the primary path. Re-routing it through its own path — the left path —
        // would replace the primary's image that just landed.
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary)) { RejectWritesTo = { PathP } };

        var applied = WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Primary, "primary.png", PathP)], 2, NoDelay);

        Assert.True(applied);
        Assert.Equal("primary.png", desktop.Images[PathL]);
        Assert.DoesNotContain(desktop.Writes, w => w is (PathL, "left.png"));
        // Retried once through its intended path, nowhere else.
        Assert.Equal(2, desktop.Writes.Count(w => w is (PathP, "left.png")));
    }

    [Fact]
    public void Write_PersistentFailedLookupAmongThree_LeavesTheUncertainMonitorsAlone()
    {
        // Filing Left, Right, Primary → painting Primary, Left, Right. The Right rect cannot be
        // read. Dropping it would compact the list to Left, Primary and file Left's image under
        // the primary path, which the shell paints on Right.
        var desktop = new FakeDesktop((PathL, Left), (PathR, Right), (PathP, Primary))
        {
            RectFailures = { [PathR] = int.MaxValue },
        };

        var applied = WindowsWallpaperService.WriteImages(desktop, [Screen(Left, "left.png", PathL)], 3, NoDelay);

        // Left is painted by the Right slot, a hole: its destination is uncertain.
        Assert.False(applied);
        Assert.Empty(desktop.Writes);
    }

    [Fact]
    public void Write_PersistentFailedLookupAmongThree_StillWritesTheCertainPairs()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathR, Right), (PathP, Primary))
        {
            RectFailures = { [PathR] = int.MaxValue },
        };

        WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Right, "right.png", PathR), Screen(Primary, "primary.png", PathP)],
            3, NoDelay);

        // Only the Left slot pairs two readable monitors: it is painted on the primary.
        Assert.Equal([(PathL, "primary.png")], desktop.Writes);
    }

    [Fact]
    public void Write_TransientFailedLookup_IsRetriedIntoAFullSnapshot()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathR, Right), (PathP, Primary))
        {
            RectFailures = { [PathR] = 1 },
        };

        WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Right, "right.png", PathR), Screen(Primary, "primary.png", PathP)],
            3, NoDelay);

        Assert.Equal("primary.png", desktop.Images[PathL]);
        Assert.Equal("left.png", desktop.Images[PathR]);
        Assert.Equal("right.png", desktop.Images[PathP]);
    }

    [Fact]
    public void Write_StaleUnreadableEntry_DropsOutWhenEveryActiveMonitorWasRead()
    {
        // A listed slot that is not on the desktop (unplugged monitor) fails its rect lookup;
        // SM_CMONITORS says the two readable slots are the whole desktop.
        var desktop = new FakeDesktop((PathL, Left), (@"\\?\DISPLAY#GONE#1", Right), (PathP, Primary))
        {
            RectFailures = { [@"\\?\DISPLAY#GONE#1"] = int.MaxValue },
        };

        WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Primary, "primary.png", PathP)], 2, NoDelay);

        Assert.Equal("primary.png", desktop.Images[PathL]);
        Assert.Equal("left.png", desktop.Images[PathP]);
    }

    [Fact]
    public void Write_ScreenWithoutASlot_FallsBackToItsDevicePath_OnlyWhenThatFilesNoSlot()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary));
        var elsewhere = new WinDef.Rect(5000, 0, 6000, 1000);

        WindowsWallpaperService.WriteImages(desktop,
        [
            Screen(Left, "left.png", PathL),
            Screen(Primary, "primary.png", PathP),
            Screen(elsewhere, "own.png", @"\\?\DISPLAY#OWN#1"),
            Screen(elsewhere, "collides.png", PathL),
        ], 2, NoDelay);

        Assert.Equal("own.png", desktop.Images[@"\\?\DISPLAY#OWN#1"]);
        Assert.DoesNotContain(desktop.Writes, w => w.Image == "collides.png");
    }

    [Fact]
    public void Write_NoEnumeration_FallsBackToEveryDevicePath()
    {
        var desktop = new FakeDesktop((PathL, Left), (PathP, Primary)) { CountFails = true };

        var applied = WindowsWallpaperService.WriteImages(desktop,
            [Screen(Left, "left.png", PathL), Screen(Primary, "primary.png", PathP)], 2, NoDelay);

        Assert.True(applied);
        Assert.Equal("left.png", desktop.Images[PathL]);
        Assert.Equal("primary.png", desktop.Images[PathP]);
    }

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

    static ScreenWallpaper Screen(WinDef.Rect rect, string image, string deviceId)
        => new(new Rect(rect.X, rect.Y, rect.Width, rect.Height), image, WallpaperStyle.Fill, null, deviceId);

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
