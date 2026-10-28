using DynamicData.Binding;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Options;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The crossing-algorithm picker is the only enum the user chooses that travels to the daemon
/// verbatim, and its list is the sole thing in the app that produces those values.
/// <para>
/// The ids are NOT display strings â€” they are written into the saved layout and into the
/// <c>Algorithm</c> attribute of the ZonesLayout XML, where the daemon matches them
/// case-sensitively (<c>rust/crates/lbm-zones/src/layout.rs</c>). Anything it does not
/// recognise it reads as "Strait", silently, because an unknown algorithm is not an error. So a
/// typo or a case change here does not fail, does not warn, and does not show up in the UI: it
/// just quietly runs the wrong algorithm.
/// </para>
/// <para>
/// This repository has already had four other spellings of this value in circulation â€” see
/// <c>wire-contract/README.md</c>. This test is the guard on the producing end; the golden
/// corpus guards the payload itself.
/// </para>
/// </summary>
public class AlgorithmChoiceTests
{
    /// <summary>The values the daemon understands, in the order the UI offers them.</summary>
    static readonly string[] WireValues = ["Strait", "Cross"];

    static LbmOptionsViewModel NewOptionsViewModel() =>
        new(new FakeProcessesCollector(), new FakeMainService(), new FakeDaemon());

    [Fact]
    public void AlgorithmListOffersExactlyTheWireValues()
    {
        var ids = NewOptionsViewModel().AlgorithmList.Select(item => item.Id).ToArray();

        // Exact and ordered: "cross" would be read as Strait, and a third entry would be a
        // value the daemon has no case for.
        Assert.Equal(WireValues, ids);
    }

    [Fact]
    public void AlgorithmIdsAreDistinctFromTheirCaptions()
    {
        // "Corner crossing" is what the user reads; "Cross" is what goes on the wire. Binding
        // the picker to the caption would send a value the daemon silently ignores, so the two
        // must not be allowed to quietly become the same field.
        var cross = NewOptionsViewModel().AlgorithmList.Single(item => item.Id == "Cross");

        Assert.Equal("Corner crossing", cross.Caption);
        Assert.NotEqual(cross.Caption, cross.Id);
        Assert.NotEmpty(cross.Description);
    }

    [Fact]
    public void DefaultAlgorithmIsOneTheListCanSelect()
    {
        // The default has to resolve to a list entry by Id, or the picker opens with nothing
        // selected and the user cannot tell which algorithm is running.
        var ids = NewOptionsViewModel().AlgorithmList.Select(item => item.Id).ToArray();

        Assert.Contains(new ILayoutOptions.Design().Algorithm, ids);
        Assert.Contains(new LbmOptions().Algorithm, ids);
    }


    [Fact]
    public async Task TouchscreenSelectionChangesTheSavedIds()
    {
        var model = new LbmOptions { TouchAllDisplays = false };
        using var vm = NewOptionsViewModel();
        vm.Model = model;
        await WaitForRows(vm, false);
        Assert.Single(vm.TouchDisplays);
        Assert.False(vm.TouchDisplays[0].Selected);
        model.Saved = true;
        vm.TouchDisplays[0].Selected = true;
        Assert.Equal("SRC1", model.TouchDisplayIds);
        Assert.False(model.Saved);
        await WaitForRows(vm, true);
        vm.TouchDisplays[0].Selected = false;
        Assert.Equal("", model.TouchDisplayIds);
        await WaitForRows(vm, false);
        model.TouchDisplayIds = "SRC1";
        await WaitForRows(vm, true);
        Assert.True(vm.TouchDisplays[0].Selected);
        Assert.Contains(model.TouchOverrideModifier, vm.TouchModifiers);
    }

    static async Task WaitForRows(LbmOptionsViewModel vm, bool selected)
    {
        for (var attempt = 0; attempt < 200; attempt++)
        {
            // The production view rebuilds its collection on the UI scheduler.
            // A headless test must await that dispatch before inspecting rows.
            if (vm.TouchDisplays.FirstOrDefault()?.Selected == selected) return;
            await Task.Delay(10);
        }
        Assert.True(false, "Touchscreen rows did not finish updating");
    }

    [Theory]
    [InlineData(-1, 0)]
    [InlineData(275, 275)]
    [InlineData(9999, 5000)]
    public void FocusDelayIsClampedAndMarksOptionsUnsaved(int delay, int expected)
    {
        var model = new LbmOptions { Saved = true };
        model.FocusRestoreDelay = delay;
        Assert.Equal(expected, model.FocusRestoreDelay);
        Assert.False(model.Saved);
    }

    [Fact]
    public void DisposedOptionsAndDisplayRowsAreCollectibleWithServicesStillAlive()
    {
        var daemon = new FakeDaemon();
        var main = new FakeMainService();
        var collector = new FakeProcessesCollector();
        var references = new List<WeakReference>();
        for (var i = 0; i < 100; i++)
            references.AddRange(CreateAndDisposeOptions(daemon, main, collector));

        GC.Collect();
        GC.WaitForPendingFinalizers();
        GC.Collect();
        Assert.False(daemon.HasSubscribers);
        Assert.All(references, reference => Assert.False(reference.IsAlive));
        GC.KeepAlive(daemon);
        GC.KeepAlive(main);
        GC.KeepAlive(collector);
    }

    [Fact]
    public void StylusOptInMarksOptionsUnsavedWithoutEnablingOtherFeatures()
    {
        var model = new LbmOptions { Saved = true };
        Assert.False(model.StylusMouseIndependent);
        model.StylusMouseIndependent = true;
        Assert.False(model.Saved);
        Assert.False(model.TouchMouseIndependent);
        Assert.False(model.RestoreKeyboardFocus);
    }

    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    static WeakReference[] CreateAndDisposeOptions(FakeDaemon daemon, FakeMainService main,
        FakeProcessesCollector collector)
    {
        var model = new LbmOptions();
        var vm = new LbmOptionsViewModel(collector, main, daemon) { Model = model };
        Assert.True(SpinWait.SpinUntil(() => vm.TouchDisplays.Count == 1, 2000));
        var firstRow = new WeakReference(vm.TouchDisplays[0]);
        // Exercise replacement of row callbacks while the options remain open.
        for (var i = 0; i < 20; i++) model.TouchDisplayIds = i % 2 == 0 ? "SRC1" : "";
        var references = new[] { new WeakReference(vm), new WeakReference(model), firstRow };
        vm.Dispose();
        return references;
    }

    sealed class FakeProcessesCollector : IProcessesCollector
    {
        public ObservableCollectionExtended<string> SeenProcesses { get; } = [];
        public void AddProcess(string process) => SeenProcesses.Add(process);
    }

    sealed class FakeMainService : IMainService
    {
        public IMonitorsLayout MonitorsLayout { get; set; } =
            MainServiceFakes.NewLayout(new ILayoutOptions.Design());

        public bool LivePreview { get; set; }

        public void UpdateLayout() { }
        public void ReloadSystemLayout() { }
        public Task StartNotifierAsync() => Task.CompletedTask;
        public Task ShowControlAsync() => Task.CompletedTask;
        public void AddControlPlugin(Action<IMainPluginsViewModel>? action) { }
    }
}
