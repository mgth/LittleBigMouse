using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Tests of the shared persistence engine over an in-memory store: everything here holds
/// identically for the registry backend (Windows) and the JSON backend (Linux), which is
/// the point of the abstraction.
/// </summary>
public class LayoutPersistenceTests
{
    const string LayoutId = "TESTMON1";
    const string Pnp = "TST1234";

    sealed class FakeStore : ILayoutStore
    {
        public GlobalOptionsDto? GlobalOptions;
        public readonly Dictionary<string, LayoutDto> Layouts = [];
        public readonly Dictionary<string, ModelDto> Models = [];

        public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes) => new()
        {
            GlobalOptions = GlobalOptions,
            Layout = Layouts.GetValueOrDefault(layoutId),
            Models = Models.Where(kv => pnpCodes.Contains(kv.Key)).ToDictionary(kv => kv.Key, kv => kv.Value)
        };

        public void WriteGlobalOptions(GlobalOptionsDto options) => GlobalOptions = options;

        public void WriteLayout(string layoutId, LayoutDto layout) => Layouts[layoutId] = layout;

        public void WriteModels(IReadOnlyDictionary<string, ModelDto> models)
        {
            foreach (var (pnpCode, model) in models) Models[pnpCode] = model;
        }
    }

    sealed class TestPersistence(ILayoutStore store, string excludedFile) : LayoutPersistence(store)
    {
        protected override string ExcludedListFile() => excludedFile;
    }

    static string TempExcludedFile()
    {
        var dir = Path.Combine(Path.GetTempPath(), "lbm-persistence-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        return Path.Combine(dir, "Excluded.txt");
    }

    static TestPersistence NewPersistence(FakeStore store) => new(store, TempExcludedFile());

    static MonitorsLayout NewLayout(out PhysicalMonitor monitor, out DisplaySource source)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design()) { Id = LayoutId };

        var model = new PhysicalMonitorModel(Pnp);
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;
        model.PhysicalSize.LeftBorder = 10;
        model.PhysicalSize.TopBorder = 11;
        model.PhysicalSize.RightBorder = 12;
        model.PhysicalSize.BottomBorder = 13;

        monitor = new PhysicalMonitor("MON1", layout, model);
        source = new DisplaySource("SRC1") { AttachedToDesktop = true };
        source.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource("DEV1", monitor, source);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return layout;
    }

    [Fact]
    public void SaveThenLoad_RoundTripsMonitorGeometry()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(out var monitor, out _);
        monitor.DepthProjection.X = 123.5;
        monitor.DepthProjection.Y = -42.25;
        monitor.DepthRatio.X = 1.25;
        monitor.DepthRatio.Y = 1.5;
        monitor.BorderResistance.Left = 1;
        monitor.BorderResistance.Top = 2;
        monitor.BorderResistance.Right = 3;
        monitor.BorderResistance.Bottom = 4;

        Assert.True(persistence.Save(layout));

        var restored = NewLayout(out var restoredMonitor, out _);
        persistence.Load(restored);

        Assert.Equal(123.5, restoredMonitor.DepthProjection.X);
        Assert.Equal(-42.25, restoredMonitor.DepthProjection.Y);
        Assert.Equal(1.25, restoredMonitor.DepthRatio.X);
        Assert.Equal(1.5, restoredMonitor.DepthRatio.Y);
        Assert.Equal(1, restoredMonitor.BorderResistance.Left);
        Assert.Equal(4, restoredMonitor.BorderResistance.Bottom);
        Assert.True(restoredMonitor.Placed);
        Assert.True(restoredMonitor.Saved);
        Assert.True(restored.Saved);
    }

    [Fact]
    public void Load_EmptyStore_LeavesDefaultsAndMarksSaved()
    {
        var persistence = NewPersistence(new FakeStore());

        var layout = NewLayout(out var monitor, out var source);
        persistence.Load(layout);

        // Nothing stored: not placed (that flag means "the user placed it"), but the whole
        // subtree must still be flagged saved so the next edit is observable.
        Assert.False(monitor.Placed);
        Assert.False(monitor.BordersCustomized);
        Assert.True(monitor.Saved);
        Assert.True(monitor.DepthProjection.Saved);
        Assert.True(source.Saved);
        Assert.True(layout.Options.Saved);
        Assert.True(layout.Saved);
    }

    [Fact]
    public void Borders_StoredOnlyWhenCustomized_AndRestoreTheFlag()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(out var monitor, out _);
        persistence.Save(layout);

        // Uncustomized: the monitor mirrors its model, nothing per-monitor is stored.
        Assert.Null(store.Layouts[LayoutId].Monitors["MON1"].Borders);

        monitor.BordersCustomized = true;
        monitor.Borders.Left = 5;
        monitor.Borders.Top = 6;
        monitor.Borders.Right = 7;
        monitor.Borders.Bottom = 8;
        persistence.Save(layout);

        Assert.NotNull(store.Layouts[LayoutId].Monitors["MON1"].Borders);

        var restored = NewLayout(out var restoredMonitor, out _);
        persistence.Load(restored);

        Assert.True(restoredMonitor.BordersCustomized);
        Assert.Equal(5, restoredMonitor.Borders.Left);
        Assert.Equal(8, restoredMonitor.Borders.Bottom);
    }

    [Fact]
    public void Load_StoredNonPositiveModelSize_DoesNotOverrideComputedSize()
    {
        var store = new FakeStore();
        store.Models[Pnp] = new ModelDto { Width = 0, Height = 0, PnpName = "Stored name" };
        var persistence = NewPersistence(store);

        var layout = NewLayout(out var monitor, out _);
        persistence.Load(layout);

        Assert.Equal(600, monitor.Model.PhysicalSize.Width);
        Assert.Equal(340, monitor.Model.PhysicalSize.Height);
        Assert.Equal("Stored name", monitor.Model.PnpDeviceName);
    }

    [Fact]
    public void Load_StoredModel_AppliesSizeAndBorders()
    {
        var store = new FakeStore();
        store.Models[Pnp] = new ModelDto
        {
            Width = 700,
            Height = 400,
            Borders = new BordersDto { Left = 20, Top = 21, Right = 22, Bottom = 23 }
        };
        var persistence = NewPersistence(store);

        var layout = NewLayout(out var monitor, out _);
        persistence.Load(layout);

        Assert.Equal(700, monitor.Model.PhysicalSize.Width);
        Assert.Equal(400, monitor.Model.PhysicalSize.Height);
        Assert.Equal(20, monitor.Model.PhysicalSize.LeftBorder);
        Assert.Equal(23, monitor.Model.PhysicalSize.BottomBorder);
    }

    [Fact]
    public void SaveEnabled_TogglesEnabledAndPreservesTheRest()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out _);
        layout.Options.Algorithm = "CornerCrossing";
        layout.Options.Enabled = true;
        persistence.Save(layout);

        layout.Options.Enabled = false;
        Assert.True(persistence.SaveEnabled(layout));

        var dto = store.Layouts[LayoutId];
        Assert.False(dto.Options!.Enabled);
        Assert.Equal("CornerCrossing", dto.Options.Algorithm);
        Assert.True(dto.Monitors.ContainsKey("MON1"));
    }

    [Fact]
    public void Load_LayoutPriority_OverridesGlobalPriority()
    {
        var store = new FakeStore
        {
            GlobalOptions = new GlobalOptionsDto { Priority = "High", PriorityUnhooked = "Idle" }
        };
        store.Layouts[LayoutId] = new LayoutDto { Options = new LayoutOptionsDto { Priority = "Realtime" } };
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        Assert.Equal("Realtime", layout.Options.Priority);
        // Not overridden per-layout: the global value applies.
        Assert.Equal("Idle", layout.Options.PriorityUnhooked);
    }

    [Fact]
    public void Load_FirstRun_SeedsExcludedDefaultsFile()
    {
        var store = new FakeStore();
        var excluded = TempExcludedFile();
        var persistence = new TestPersistence(store, excluded);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        Assert.Equal(ExcludedProcessDefaults.All, layout.Options.ExcludedList);
        Assert.Equal(ExcludedProcessDefaults.All, File.ReadAllLines(excluded));
    }

    [Fact]
    public void Load_ExcludedDefaults_ToppedUpOnce()
    {
        var store = new FakeStore();
        var excluded = TempExcludedFile();
        File.WriteAllLines(excluded, ExcludedProcessDefaults.LegacyV0);
        var persistence = new TestPersistence(store, excluded);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        // The list kept all previous defaults: new defaults are added, file included,
        // and the applied version is recorded in the store.
        Assert.Contains(@"\XboxGames\", layout.Options.ExcludedList);
        Assert.Contains(@"\XboxGames\", File.ReadAllLines(excluded));
        Assert.Equal(ExcludedProcessDefaults.Version, store.GlobalOptions?.ExcludedDefaultsVersion);
    }

    [Fact]
    public void Load_ExcludedDefaults_CustomizedListLeftUntouched()
    {
        var store = new FakeStore();
        var excluded = TempExcludedFile();
        // A legacy default was removed by the user: no top-up, but the version is
        // recorded so the migration never runs again.
        File.WriteAllLines(excluded, [@"\steamapps\", @"\my\own\path\"]);
        var persistence = new TestPersistence(store, excluded);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        Assert.DoesNotContain(@"\XboxGames\", layout.Options.ExcludedList);
        Assert.Equal(ExcludedProcessDefaults.Version, store.GlobalOptions?.ExcludedDefaultsVersion);
    }

    [Fact]
    public void Load_ExcludedDefaults_AlreadyApplied_DoesNothing()
    {
        var store = new FakeStore
        {
            GlobalOptions = new GlobalOptionsDto { ExcludedDefaultsVersion = ExcludedProcessDefaults.Version }
        };
        var excluded = TempExcludedFile();
        File.WriteAllLines(excluded, ExcludedProcessDefaults.LegacyV0);
        var persistence = new TestPersistence(store, excluded);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        Assert.DoesNotContain(@"\XboxGames\", layout.Options.ExcludedList);
    }

    [Fact]
    public void Load_DetachedSource_RestoresStoredGeometry()
    {
        var store = new FakeStore();
        store.Layouts[LayoutId] = new LayoutDto
        {
            Monitors =
            {
                ["MON1"] = new MonitorDto
                {
                    ActiveSource = "SRC1",
                    Sources = new Dictionary<string, SourceDto>
                    {
                        ["SRC1"] = new() { PixelX = 100, PixelY = 200, PixelWidth = 800, PixelHeight = 600, Orientation = 1 }
                    }
                }
            }
        };
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out var source);
        source.AttachedToDesktop = false;
        persistence.Load(layout);

        Assert.Equal(100, source.InPixel.X);
        Assert.Equal(200, source.InPixel.Y);
        Assert.Equal(800, source.InPixel.Width);
        Assert.Equal(600, source.InPixel.Height);
        Assert.Equal(1, source.Orientation);
    }

    [Fact]
    public void Load_AttachedSource_KeepsLiveGeometry()
    {
        var store = new FakeStore();
        store.Layouts[LayoutId] = new LayoutDto
        {
            Monitors =
            {
                ["MON1"] = new MonitorDto
                {
                    Sources = new Dictionary<string, SourceDto>
                    {
                        ["SRC1"] = new() { PixelX = 100, PixelY = 200, PixelWidth = 800, PixelHeight = 600 }
                    }
                }
            }
        };
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out var source);
        persistence.Load(layout);

        // Attached: the live geometry wins, the store is just a backup for re-attach.
        Assert.Equal(0, source.InPixel.X);
        Assert.Equal(1920, source.InPixel.Width);
    }

    [Fact]
    public void Save_StoresAttachedSourcesOnly()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(out _, out var source);
        source.AttachedToDesktop = false;
        persistence.Save(layout);

        Assert.Empty(store.Layouts[LayoutId].Monitors["MON1"].Sources!);
    }

    [Fact]
    public void SaveLive_WritesGlobalOptionsAndExcludedFile()
    {
        var store = new FakeStore();
        var excluded = TempExcludedFile();
        var persistence = new TestPersistence(store, excluded);

        var layout = NewLayout(out _, out _);
        persistence.Load(layout);

        layout.Options.HideTrayIcon = true;
        layout.Options.ExcludedList.Add(@"\my\game\");
        persistence.SaveLive(layout.Options);

        Assert.True(store.GlobalOptions?.HideTrayIcon);
        // The version survives a live save even though it is not part of the options model.
        Assert.Equal(ExcludedProcessDefaults.Version, store.GlobalOptions?.ExcludedDefaultsVersion);
        Assert.Contains(@"\my\game\", File.ReadAllLines(excluded));
    }

    [Fact]
    public void Load_TransposesPre541OrientedStoredSize()
    {
        // Pre-5.4.1 the model persisted the size ORIENTED to the rotation at save time:
        // a monitor portrait at save time stored a transposed size. Read as intrinsic
        // (5.4.1 semantics) it gets the rotation applied twice — the #507 follow-up where
        // the previously-portrait monitor renders inverted after the upgrade. The stored
        // value must be transposed back, keeping its (possibly user-customized) magnitudes.
        var store = new FakeStore();
        store.Models[Pnp] = new ModelDto { Width = 338, Height = 598 };

        var layout = NewLayout(out var monitor, out _); // fresh intrinsic model: 600x340
        NewPersistence(store).Load(layout);

        Assert.Equal(598, monitor.Model.PhysicalSize.Width);
        Assert.Equal(338, monitor.Model.PhysicalSize.Height);
    }

    [Fact]
    public void Load_KeepsStoredSizeWhenOrientationMatches()
    {
        var store = new FakeStore();
        store.Models[Pnp] = new ModelDto { Width = 598, Height = 338 };

        var layout = NewLayout(out var monitor, out _); // fresh intrinsic model: 600x340
        NewPersistence(store).Load(layout);

        Assert.Equal(598, monitor.Model.PhysicalSize.Width);
        Assert.Equal(338, monitor.Model.PhysicalSize.Height);
    }

    [Theory]
    // Square intrinsic or stored size: orientation is undecidable, keep as stored.
    [InlineData(500, 500, 340, 600, 340, 600)]
    [InlineData(600, 340, 500, 500, 500, 500)]
    // Invalid intrinsic reference (EDID-less placeholder): keep as stored.
    [InlineData(0, 0, 340, 600, 340, 600)]
    // Contradicting orientation: transpose, magnitudes preserved.
    [InlineData(600, 340, 340, 600, 600, 340)]
    // Intrinsically-portrait panel, stored landscape (saved while rotated): transpose too.
    [InlineData(340, 600, 600, 340, 340, 600)]
    public void NormalizeStoredSize_Cases(
        double intrinsicW, double intrinsicH, double storedW, double storedH,
        double expectedW, double expectedH)
    {
        var (w, h) = LayoutPersistence.NormalizeStoredSize(intrinsicW, intrinsicH, storedW, storedH);
        Assert.Equal(expectedW, w);
        Assert.Equal(expectedH, h);
    }
}
