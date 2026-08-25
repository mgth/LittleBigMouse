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
/// The ids are NOT display strings — they are written into the saved layout and into the
/// <c>Algorithm</c> attribute of the ZonesLayout XML, where the daemon matches them
/// case-sensitively (<c>LittleBigMouse-Hook-Rust/src/zones/layout.rs</c>). Anything it does not
/// recognise it reads as "Strait", silently, because an unknown algorithm is not an error. So a
/// typo or a case change here does not fail, does not warn, and does not show up in the UI: it
/// just quietly runs the wrong algorithm.
/// </para>
/// <para>
/// This repository has already had four other spellings of this value in circulation — see
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
