using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Linux;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// End-to-end golden tests of the persistence engine over REAL files: the fixtures under
/// <c>TestData/Persistence</c> are configuration directories as the supported versions
/// wrote them, read through the actual Linux store (<see cref="JsonLayoutStore"/>) and
/// mapped by <see cref="LayoutPersistence"/>.
/// <para>
/// What they lock, which the in-memory tests cannot: the JSON property NAMES, the shapes
/// a stored document may take across versions, and the exact bytes a save produces. A DTO
/// rename, a changed default or a migration silently dropped all fail here, with the
/// fixture showing what a user's file actually looks like.
/// </para>
/// <para>
/// Windows stores the same DTOs in the registry (<c>RegistryLayoutStore</c>), which cannot
/// be exercised on this platform; the engine, the mapping and the migrations under test
/// are the shared ones, so only the storage encoding is left uncovered here.
/// </para>
/// <para>
/// Set <c>LBM_UPDATE_GOLDEN=1</c> to rewrite the expected save output after an
/// INTENTIONAL format change, then read the diff before committing it: an unexpected line
/// there is a user's configuration being orphaned.
/// </para>
/// </summary>
public class LayoutPersistenceGoldenTests : IDisposable
{
    // The fixtures are named after these: a layout id built from the monitor combination,
    // one monitor of model TST1234 with one attached source.
    const string LayoutId = "TST1234_1920x1080";
    const string Pnp = "TST1234";
    const string MonitorId = "TST1234_0";
    const string SourceId = "DISPLAY1";

    readonly string _work = Path.Combine(
        Path.GetTempPath(), "lbm-golden-tests", Guid.NewGuid().ToString("N"));

    public void Dispose()
    {
        try { Directory.Delete(_work, true); } catch { }
    }

    //==================//
    // Supported versions

    [Fact]
    public void Load_v52_MigratesWholeEdgeResistanceAndOrientedModelSize()
    {
        var (layout, monitor, _) = Load("v5.2-pre-sections");

        // Options that lived in the store before 5.5 arrive unchanged...
        Assert.Equal(25196, layout.Options.DaemonPort);
        Assert.Equal("High", layout.Options.Priority);
        Assert.Equal("Idle", layout.Options.PriorityUnhooked);
        Assert.True(layout.Options.Pinned);
        Assert.Equal("PerModel", layout.Options.BorderValues);
        // ...and options that did not exist yet keep their current default.
        Assert.False(layout.Options.VcpControl);
        Assert.False(layout.Options.HideTrayIcon);
        Assert.Equal("Ctrl+Alt+Shift+M", layout.Options.RescueShortcut);
        Assert.True(layout.Options.FreelookEnabled);

        // Pre-5.4.1 stored the size ORIENTED to the rotation at save time (#507): the
        // portrait 338x598 contradicts the intrinsic landscape panel and is transposed.
        Assert.Equal(598, monitor.Model.PhysicalSize.Width);
        Assert.Equal(338, monitor.Model.PhysicalSize.Height);
        Assert.Equal(8, monitor.Model.PhysicalSize.LeftBorder);
        Assert.Equal(14, monitor.Model.PhysicalSize.BottomBorder);
        Assert.Equal("Test Monitor 24", monitor.Model.PnpDeviceName);

        Assert.Equal(320.5, monitor.DepthProjection.X);
        Assert.Equal(-12, monitor.DepthProjection.Y);
        Assert.True(monitor.Placed);

        // One resistance per edge, the only shape that existed before the section
        // editor: it becomes the section that says the same thing, spanning the edge.
        foreach (var (side, length) in new[]
                 {
                     (monitor.BorderResistance.Left, monitor.DepthProjection.Height),
                     (monitor.BorderResistance.Right, monitor.DepthProjection.Height)
                 })
        {
            var section = Assert.Single(side.Sections.Items);
            Assert.Equal(0, section.From);
            Assert.Equal(length, section.To);
            Assert.Equal(20, section.Move);
            Assert.Equal(20, section.Drag);
            Assert.False(section.MoveBlock);
            Assert.False(section.DragBlock);
        }

        // A zero edge is what "no resistance" always looked like: no section for it.
        Assert.Empty(monitor.BorderResistance.Top.Sections.Items);
        Assert.Empty(monitor.BorderResistance.Bottom.Sections.Items);

        // No per-monitor Borders in the document: the monitor still mirrors its model.
        Assert.False(monitor.BordersCustomized);
        Assert.Equal(8, monitor.Borders.Left);
    }

    [Fact]
    public void Load_v55_ReadsSectionsAndPerMonitorBorders()
    {
        var (layout, monitor, _) = Load("v5.5-sections");

        Assert.True(layout.Options.VcpControl);
        Assert.Equal("PerMonitor", layout.Options.BorderValues);
        // The layout document overrides the app-level priorities, and only those.
        Assert.Equal("Realtime", layout.Options.Priority);
        Assert.Equal("Idle", layout.Options.PriorityUnhooked);
        Assert.Equal("CornerCrossing", layout.Options.Algorithm);
        Assert.Equal(150, layout.Options.MaxTravelDistance);
        Assert.True(layout.Options.LoopX);

        // Post-5.4.1: the stored size is intrinsic already, nothing to transpose.
        Assert.Equal(598, monitor.Model.PhysicalSize.Width);
        Assert.Equal(338, monitor.Model.PhysicalSize.Height);

        var left = Assert.Single(monitor.BorderResistance.Left.Sections.Items);
        Assert.Equal(0, left.From);
        Assert.Equal(338, left.To);
        Assert.Equal(20, left.Move);

        var right = Assert.Single(monitor.BorderResistance.Right.Sections.Items);
        Assert.Equal(50, right.From);
        Assert.Equal(150, right.To);
        Assert.Equal(5, right.Move);
        Assert.True(right.MoveBlock);
        Assert.Equal(0, right.Drag);

        // Sections present: the sibling Move/Drag of the same edge are NOT migrated on
        // top of them (they are what the section list replaced).
        Assert.Equal(1, monitor.BorderResistance.Left.Sections.Items.Count);

        Assert.True(monitor.BordersCustomized);
        Assert.Equal(5, monitor.Borders.Left);
        Assert.Equal(8, monitor.Borders.Bottom);
        Assert.True(monitor.ExcludedFromLayout);
    }

    [Fact]
    public void Load_v56_ReadsEveryCurrentField()
    {
        var (layout, monitor, source) = Load("v5.6-current");

        Assert.True(layout.Options.HideTrayIcon);
        Assert.True(layout.Options.Pinned);
        Assert.False(layout.Options.AutoUpdate);
        Assert.Equal("Ctrl+Alt+Shift+M", layout.Options.RescueShortcut);

        Assert.Equal(2, monitor.BorderResistance.Left.Sections.Items.Count);
        var second = monitor.BorderResistance.Left.Sections.Items[1];
        Assert.Equal(120, second.From);
        Assert.Equal(338, second.To);
        Assert.True(second.MoveBlock);
        Assert.True(second.DragBlock);

        Assert.Empty(monitor.BorderResistance.Top.Sections.Items);
        Assert.False(monitor.ExcludedFromLayout);

        // The source is attached: the live geometry wins over the stored backup.
        Assert.Equal(1920, source.InPixel.Width);
        Assert.Equal(SourceId, monitor.ActiveSource.Source.Id);
    }

    //==================//
    // Round trip

    [Fact]
    public void SaveAfterLoad_v56_ReproducesTheStoredFiles()
    {
        var dir = Fixture("v5.6-current");
        var store = new JsonLayoutStore(dir);
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);
        Assert.True(persistence.Save(layout));

        // A save right after a load must not change what the user has: every byte the
        // current version writes is compared against the committed expectation.
        //
        // Two differences from the loaded document are visible in the golden:
        //  - the layout's Priority/PriorityUnhooked moved to options.json, which is the
        //    one place they are stored now — see
        //    SaveAfterLoad_PromotesTheLayoutPriorityToTheAppLevelAndDropsIt.
        //  - RescueShortcut comes back with every '+' written as a unicode escape: that
        //    is the default JSON encoder, and the string parses back identically, so a
        //    hand-edited file keeps working. Only the bytes differ.
        AssertGolden("v5.6-current-saved/options.json", Path.Combine(dir, "options.json"));
        AssertGolden("v5.6-current-saved/models.json", Path.Combine(dir, "models.json"));
        AssertGolden($"v5.6-current-saved/layouts/{LayoutId}.json",
            Path.Combine(dir, "layouts", $"{LayoutId}.json"));
    }

    [Fact]
    public void SaveAfterLoad_v52_UpgradesTheDocumentToTheCurrentShape()
    {
        var dir = Fixture("v5.2-pre-sections");
        var store = new JsonLayoutStore(dir);
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);
        persistence.Save(layout);

        // The migrations only reach the store here — and once written, re-reading them
        // is a no-op: the bare per-edge numbers are gone, replaced by the sections that
        // carry the same setting, and the transposed size is now intrinsic.
        AssertGolden("v5.2-pre-sections-saved/models.json", Path.Combine(dir, "models.json"));
        AssertGolden($"v5.2-pre-sections-saved/layouts/{LayoutId}.json",
            Path.Combine(dir, "layouts", $"{LayoutId}.json"));

        // Reloading the upgraded document yields the same model.
        var reloaded = NewLayout(out var monitor, out _);
        NewPersistence(new JsonLayoutStore(dir)).Load(reloaded);

        var section = Assert.Single(monitor.BorderResistance.Left.Sections.Items);
        Assert.Equal(0, section.From);
        Assert.Equal(monitor.DepthProjection.Height, section.To);
        Assert.Equal(20, section.Move);
        Assert.Equal(598, monitor.Model.PhysicalSize.Width);
    }

    [Fact]
    public void SaveAfterLoad_PromotesTheLayoutPriorityToTheAppLevelAndDropsIt()
    {
        // A layout carrying its own Priority still wins at load — that is what keeps an
        // upgrade from changing anybody's setting. The save then stores it once, at the
        // app level, and the layout copy is gone: the two locations were never two
        // settings (both load into the same options property), and keeping the copy is
        // what used to hand one layout's value to all the others.
        var dir = Fixture("v5.6-current");
        var store = new JsonLayoutStore(dir);

        var layout = NewLayout(out _, out _);
        var persistence = NewPersistence(store);
        persistence.Load(layout);

        Assert.Equal("High", ReadFixtureGlobalPriority("v5.6-current"));
        Assert.Equal("Realtime", layout.Options.Priority);

        persistence.Save(layout);

        var saved = store.Read(LayoutId, []);
        Assert.Equal("Realtime", saved.GlobalOptions!.Priority);
        Assert.Equal("Idle", saved.GlobalOptions.PriorityUnhooked);
        Assert.Null(saved.Layout!.Options!.Priority);
        Assert.Null(saved.Layout.Options.PriorityUnhooked);

        // And it stays there: a reload finds one value, in one place.
        var reloaded = NewLayout(out _, out _);
        NewPersistence(new JsonLayoutStore(dir)).Load(reloaded);
        Assert.Equal("Realtime", reloaded.Options.Priority);
    }

    [Fact]
    public void SaveEnabled_DoesNotPutTheLayoutPriorityBack()
    {
        // SaveEnabled rewrites the document it just read, so without care it would
        // restore the legacy keys a full save had migrated away.
        var dir = Fixture("v5.6-current");
        var store = new JsonLayoutStore(dir);

        var layout = NewLayout(out _, out _);
        var persistence = NewPersistence(store);
        persistence.Load(layout);

        layout.Options.Enabled = false;
        Assert.True(persistence.SaveEnabled(layout));

        var saved = store.Read(LayoutId, []).Layout!;
        Assert.False(saved.Options!.Enabled);
        Assert.Null(saved.Options.Priority);
        // Everything else the document held is still there.
        Assert.Equal("CornerCrossing", saved.Options.Algorithm);
        Assert.True(saved.Monitors.ContainsKey(MonitorId));
    }

    static string? ReadFixtureGlobalPriority(string fixture) =>
        new JsonLayoutStore(Path.Combine(AppContext.BaseDirectory, "TestData", "Persistence", fixture))
            .Read(LayoutId, []).GlobalOptions?.Priority;

    //==================//
    // Incomplete data

    [Fact]
    public void Load_EmptyDocuments_KeepEveryDefault()
    {
        var (layout, monitor, source) = Load("incomplete-empty");

        // "{}" everywhere is indistinguishable from a missing file: the live model wins.
        Assert.Equal(25196, layout.Options.DaemonPort);
        Assert.Equal("Normal", layout.Options.Priority);
        Assert.Equal("Strait", layout.Options.Algorithm);
        Assert.True(layout.Options.Enabled);

        Assert.Equal(600, monitor.Model.PhysicalSize.Width);
        Assert.Equal(340, monitor.Model.PhysicalSize.Height);
        Assert.Equal("Live name", monitor.Model.PnpDeviceName);

        Assert.False(monitor.Placed);
        Assert.False(monitor.BordersCustomized);
        Assert.Empty(monitor.BorderResistance.Left.Sections.Items);
        Assert.Equal(1920, source.InPixel.Width);

        // Nothing stored is still a complete load: the subtree must be flagged saved so
        // the next edit is an observable transition.
        Assert.True(monitor.Saved);
        Assert.True(layout.Saved);
    }

    [Fact]
    public void Load_PartialDocuments_FillWhatIsThereAndKeepTheRest()
    {
        var (layout, monitor, source) = Load("incomplete-partial");

        // An explicit null is "absent", exactly like a missing property, and an unknown
        // property is ignored rather than fatal (a file written by a newer version).
        Assert.Equal("Normal", layout.Options.Priority);
        Assert.Equal("PerMonitor", layout.Options.BorderValues);
        Assert.False(layout.Options.Enabled);

        // Stored 0x0: the EDID-less placeholder older versions persisted (#419) must
        // never override the freshly computed size. An empty PnpName is not a name.
        Assert.Equal(600, monitor.Model.PhysicalSize.Width);
        Assert.Equal(340, monitor.Model.PhysicalSize.Height);
        Assert.Equal("Live name", monitor.Model.PnpDeviceName);

        // Half a location is still a placement; the missing half keeps the live value.
        Assert.Equal(12.5, monitor.DepthProjection.X);
        Assert.True(monitor.Placed);

        // An explicitly empty section list clears the edge; a zero, unblocked legacy
        // pair migrates to nothing; an absent edge is left as the model has it.
        Assert.Empty(monitor.BorderResistance.Right.Sections.Items);
        Assert.Empty(monitor.BorderResistance.Bottom.Sections.Items);
        Assert.Empty(monitor.BorderResistance.Left.Sections.Items);

        // Presence of Borders is the "monitor owns them" flag, even partial: the sides
        // it does not name keep mirroring the model.
        Assert.True(monitor.BordersCustomized);
        Assert.Equal(5, monitor.Borders.Left);
        Assert.Equal(11, monitor.Borders.Top);
        Assert.Equal(13, monitor.Borders.Bottom);

        // A stored source this machine no longer has, and a stored monitor absent from
        // the layout, are both ignored — the live geometry is untouched.
        Assert.Equal(0, source.InPixel.X);
        Assert.Equal(1920, source.InPixel.Width);
        Assert.Single(layout.PhysicalMonitors);
    }

    [Fact]
    public void Load_UnknownModelsInTheStore_AreNotApplied()
    {
        // models.json holds a second model (OTHER99) belonging to another machine's
        // monitor: the store is asked for the PnP codes present, and nothing else.
        var (_, monitor, _) = Load("incomplete-partial");

        Assert.Equal(Pnp, monitor.Model.PnpCode);
        Assert.NotEqual(700, monitor.Model.PhysicalSize.Width);
    }

    //==================//
    // Excluded list

    [Fact]
    public void Load_ExcludedDefaultsTopUp_RewritesTheStoredOptionsWithoutLosingThem()
    {
        var dir = Fixture("v5.5-sections");
        var excluded = Path.Combine(_work, "excluded", "Excluded.txt");
        Directory.CreateDirectory(Path.GetDirectoryName(excluded)!);
        File.WriteAllLines(excluded, ExcludedProcessDefaults.LegacyV0);

        var layout = NewLayout(out _, out _);
        new TestPersistence(new JsonLayoutStore(dir), excluded).Load(layout);

        // The stored version (1) is behind: the top-up runs and records itself, which is
        // a WRITE during a load. It goes through the document as read, so everything
        // else stored keeps its value.
        var options = new JsonLayoutStore(dir).Read(LayoutId, []).GlobalOptions!;
        Assert.Equal(ExcludedProcessDefaults.Version, options.ExcludedDefaultsVersion);
        Assert.True(options.VcpControl);
        Assert.True(options.StartMinimized);
        Assert.Equal("PerMonitor", options.BorderValues);
        Assert.Equal(25196, options.DaemonPort);

        foreach (var entry in ExcludedProcessDefaults.All)
        {
            Assert.True(ExcludedProcessDefaults.ContainsEntry(layout.Options.ExcludedList, entry),
                $"missing default: {entry}");
            Assert.True(ExcludedProcessDefaults.ContainsEntry(File.ReadAllLines(excluded), entry),
                $"missing default in the file the daemon reads: {entry}");
        }
    }

    [Fact]
    public void Load_ExcludedFileCommentLines_StayOutOfTheListAndSurviveASave()
    {
        // A real file as CreateExcludedFile seeds it, plus a line the user added by hand.
        // The daemon skips ':' lines and empty ones (daemon::load_excluded), so neither is
        // an exclusion — but neither may be lost when the app rewrites the file either.
        var dir = Fixture("v5.6-current"); // ExcludedDefaultsVersion 2: no top-up interference
        var excluded = Path.Combine(_work, "commented", "Excluded.txt");
        Directory.CreateDirectory(Path.GetDirectoryName(excluded)!);
        File.WriteAllLines(excluded,
            [ExcludedProcessDefaults.Header, .. ExcludedProcessDefaults.All, "", ":my own note"]);

        var layout = NewLayout(out _, out _);
        var persistence = new TestPersistence(new JsonLayoutStore(dir), excluded);
        persistence.Load(layout);

        Assert.Equal(ExcludedProcessDefaults.All, layout.Options.ExcludedList);

        persistence.SaveLive(layout.Options);

        var written = File.ReadAllLines(excluded);
        Assert.Equal(ExcludedProcessDefaults.Header, written[0]);
        Assert.Equal(":my own note", written[1]);
        Assert.Equal(ExcludedProcessDefaults.All, written[2..]);
    }

    //==================//
    // Fixtures         //
    //==================//

    sealed class TestPersistence(ILayoutStore store, string excludedFile) : LayoutPersistence(store)
    {
        protected override string ExcludedListFile() => excludedFile;
    }

    /// <summary>A persistence whose excluded list is a fresh file outside the fixture.</summary>
    TestPersistence NewPersistence(ILayoutStore store) => new(store,
        Path.Combine(_work, Guid.NewGuid().ToString("N"), "Excluded.txt"));

    (MonitorsLayout Layout, PhysicalMonitor Monitor, DisplaySource Source) Load(string fixture)
    {
        var layout = NewLayout(out var monitor, out var source);
        NewPersistence(new JsonLayoutStore(Fixture(fixture))).Load(layout);
        return (layout, monitor, source);
    }

    /// <summary>
    /// A writable copy of a fixture: loading may migrate and saving certainly writes,
    /// and the committed fixture must stay the file the old version wrote.
    /// </summary>
    string Fixture(string name)
    {
        var source = Path.Combine(AppContext.BaseDirectory, "TestData", "Persistence", name);
        var target = Path.Combine(_work, name);

        foreach (var file in Directory.GetFiles(source, "*", SearchOption.AllDirectories))
        {
            var copy = Path.Combine(target, Path.GetRelativePath(source, file));
            Directory.CreateDirectory(Path.GetDirectoryName(copy)!);
            File.Copy(file, copy);
        }

        return target;
    }

    /// <summary>
    /// The layout this machine would build for the fixtures: one landscape monitor,
    /// intrinsic 600x340 with 10/11/12/13 bezels, one attached 1920x1080 source.
    /// </summary>
    static MonitorsLayout NewLayout(out PhysicalMonitor monitor, out DisplaySource source)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design()) { Id = LayoutId };

        var model = new PhysicalMonitorModel(Pnp) { PnpDeviceName = "Live name" };
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;
        model.PhysicalSize.LeftBorder = 10;
        model.PhysicalSize.TopBorder = 11;
        model.PhysicalSize.RightBorder = 12;
        model.PhysicalSize.BottomBorder = 13;

        monitor = new PhysicalMonitor(MonitorId, layout, model) { SerialNumber = "SN-0001" };

        source = new DisplaySource(SourceId)
        {
            AttachedToDesktop = true,
            DisplayName = @"\\.\DISPLAY1",
            Primary = true
        };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource("DEV1", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return layout;
    }

    static void AssertGolden(string goldenRelativePath, string actualFile)
    {
        var golden = Path.Combine(AppContext.BaseDirectory, "TestData", "Persistence", goldenRelativePath);
        var actual = Normalize(File.ReadAllText(actualFile));

        if (Environment.GetEnvironmentVariable("LBM_UPDATE_GOLDEN") == "1")
        {
            // The copy in the build output is what the test reads; the source tree is
            // what gets committed, so write both.
            File.WriteAllText(golden, actual);
            var sourceTree = golden.Replace(
                Path.Combine("bin", "Debug", "net10.0") + Path.DirectorySeparatorChar, "");
            if (File.Exists(sourceTree)) File.WriteAllText(sourceTree, actual);
        }

        Assert.True(File.Exists(golden), $"missing golden file: {goldenRelativePath}");
        Assert.Equal(Normalize(File.ReadAllText(golden)), actual);
    }

    static string Normalize(string text) => text.Replace("\r\n", "\n");
}
