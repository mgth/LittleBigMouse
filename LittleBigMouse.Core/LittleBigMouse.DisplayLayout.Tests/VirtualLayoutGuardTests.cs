using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// A virtual (foreign) layout must never touch this machine's state: the persistence
/// layer refuses to store or load it, and the zones sent to the daemon carry the
/// Virtual flag the daemon keys its own hook refusal on. These guards are what keeps
/// a client's exported configuration from leaking into the local store, the autostart
/// scheduling or the crash-recovery file.
/// </summary>
public class VirtualLayoutGuardTests
{
    const string LayoutId = "CLIENTMON1";

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

    static TestPersistence NewPersistence(FakeStore store)
    {
        var dir = Path.Combine(Path.GetTempPath(), "lbm-virtual-guard-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(dir);
        return new TestPersistence(store, Path.Combine(dir, "Excluded.txt"));
    }

    static MonitorsLayout NewLayout(LayoutSource source)
    {
        var layout = new MonitorsLayout(new ILayoutOptions.Design())
        {
            Id = LayoutId,
            Source = source,
            SourceOrigin = source == LayoutSource.System ? null : "client-export.json"
        };

        var model = new PhysicalMonitorModel("TST1234");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;

        var monitor = new PhysicalMonitor("MON1", layout, model);
        var displaySource = new DisplaySource("SRC1") { AttachedToDesktop = true };
        displaySource.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource("DEV1", monitor, displaySource);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return layout;
    }

    [Fact]
    public void Save_VirtualLayout_RefusedAndStoreUntouched()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(LayoutSource.VirtualImport);
        Assert.False(persistence.Save(layout));

        Assert.Empty(store.Layouts);
        Assert.Empty(store.Models);
        Assert.Null(store.GlobalOptions);
    }

    [Fact]
    public void SaveEnabled_VirtualLayout_RefusedAndCreatesNoStoreEntry()
    {
        // The regression that motivated the guard: SaveEnabled runs on every Stop and
        // used to CREATE a store entry named after the client's monitor combination.
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(LayoutSource.VirtualFile);
        layout.Options.Enabled = false;

        Assert.False(persistence.SaveEnabled(layout));
        Assert.Empty(store.Layouts);
    }

    [Fact]
    public void Load_VirtualLayout_RefusedAndKeepsImportedState()
    {
        // A LOCAL layout may share the client's id (same monitor models): loading would
        // apply that stored state over the imported one. The import stays authoritative.
        var store = new FakeStore();
        store.Layouts[LayoutId] = new LayoutDto
        {
            Options = new LayoutOptionsDto { Algorithm = "Cross", LoopX = true }
        };
        var persistence = NewPersistence(store);

        var layout = NewLayout(LayoutSource.VirtualImport);
        persistence.Load(layout);

        Assert.Equal("Strait", layout.Options.Algorithm);
        Assert.False(layout.Options.LoopX);
    }

    [Fact]
    public void ComputeZones_CarriesTheVirtualFlagOnTheWire()
    {
        var virtualZones = NewLayout(LayoutSource.VirtualImport).ComputeZones();
        Assert.True(virtualZones.Virtual);
        Assert.Contains(@"Virtual=""True""", virtualZones.Serialize());

        var systemZones = NewLayout(LayoutSource.System).ComputeZones();
        Assert.False(systemZones.Virtual);
        Assert.Contains(@"Virtual=""False""", systemZones.Serialize());
    }

    [Fact]
    public void SystemLayout_IsNotVirtual_AndStillSaves()
    {
        var store = new FakeStore();
        var persistence = NewPersistence(store);

        var layout = NewLayout(LayoutSource.System);
        Assert.False(layout.IsVirtual);
        Assert.True(persistence.Save(layout));
        Assert.True(store.Layouts.ContainsKey(LayoutId));
    }
}
