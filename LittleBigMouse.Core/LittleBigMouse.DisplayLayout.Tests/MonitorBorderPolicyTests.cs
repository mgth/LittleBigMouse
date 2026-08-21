using System.Reactive;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// The "Border values" rules on their own: which size a monitor's geometry roots at, and how a
/// monitor comes to own its bezel borders. No monitor, layout or display source is built here —
/// that is what pulling these rules out of <see cref="PhysicalMonitor"/> bought.
/// <para>
/// <see cref="DimensionReactiveContractTests.MonitorGeometryRewiresWhenTheEffectiveDimensionInstanceChanges"/>
/// remains the end-to-end guard that a real monitor's geometry follows what this policy publishes.
/// </para>
/// </summary>
public class MonitorBorderPolicyTests
{
    sealed class ReactiveLayoutOptions : ILayoutOptions.Design
    {
        public void SetBorderValues(string value)
        {
            BorderValues = value;
            OnPropertyChanged(nameof(BorderValues));
        }
    }

    static DisplaySizeInMm ModelSize() => new()
    {
        Width = 600,
        Height = 340,
        LeftBorder = 10,
        TopBorder = 11,
        RightBorder = 12,
        BottomBorder = 13,
    };

    static (MonitorBorderPolicy Policy, DisplaySizeInMm Model, ReactiveLayoutOptions Options) Build(
        string mode = MonitorBorderPolicy.PerModel)
    {
        var model = ModelSize();
        var options = new ReactiveLayoutOptions { BorderValues = mode };
        return (new MonitorBorderPolicy(model, options), model, options);
    }

    [Fact]
    public void BordersAreSeededFromTheModelSoPerMonitorStartsMatchingPerModel()
    {
        var (policy, model, _) = Build();

        Assert.Equal(model.LeftBorder, policy.Borders.Left);
        Assert.Equal(model.TopBorder, policy.Borders.Top);
        Assert.Equal(model.RightBorder, policy.Borders.Right);
        Assert.Equal(model.BottomBorder, policy.Borders.Bottom);
        Assert.False(policy.Customized);
    }

    [Fact]
    public void EffectiveSizeIsTheModelSizeUntilTheModeSwitchesToPerMonitor()
    {
        var (policy, model, options) = Build();

        var sizes = new List<IMutableDisplaySize>();
        using var subscription = policy.EffectiveSize.Subscribe(sizes.Add);

        Assert.Same(model, Assert.Single(sizes));

        options.SetBorderValues(MonitorBorderPolicy.PerMonitor);
        Assert.Equal(2, sizes.Count);
        Assert.NotSame(model, sizes[1]);

        options.SetBorderValues(MonitorBorderPolicy.PerModel);
        Assert.Same(model, sizes[2]);

        // Back to PerMonitor must reuse the very same override instance: the geometry chain
        // carries layout-computed positions on it.
        options.SetBorderValues(MonitorBorderPolicy.PerMonitor);
        Assert.Same(sizes[1], sizes[3]);
    }

    [Fact]
    public void RepeatedModeWritesOfTheSameValueDoNotRepublishTheSize()
    {
        var (policy, _, options) = Build();

        var sizes = new List<IMutableDisplaySize>();
        using var subscription = policy.EffectiveSize.Subscribe(sizes.Add);

        options.SetBorderValues(MonitorBorderPolicy.PerModel);
        options.SetBorderValues(MonitorBorderPolicy.PerModel);

        Assert.Single(sizes);
    }

    [Fact]
    public void LateSubscribersReceiveTheCurrentSizeRatherThanWaitingForTheNextSwitch()
    {
        var (policy, _, options) = Build();

        // The first subscriber is what connects the replayed stream; the geometry chains that
        // subscribe afterwards must not miss the emission that connection already produced.
        using var first = policy.EffectiveSize.Subscribe(_ => { });
        options.SetBorderValues(MonitorBorderPolicy.PerMonitor);

        var late = new List<IMutableDisplaySize>();
        using var second = policy.EffectiveSize.Subscribe(late.Add);

        Assert.Same(policy.Borders, Assert.IsType<DisplayBorderOverride>(Assert.Single(late)).BorderSource);
    }

    [Fact]
    public void ThePerMonitorSizeKeepsTheModelDimensionsButSubstitutesTheMonitorBorders()
    {
        var (policy, model, options) = Build(MonitorBorderPolicy.PerMonitor);

        var sizes = new List<IMutableDisplaySize>();
        using var subscription = policy.EffectiveSize.Subscribe(sizes.Add);

        policy.Borders.Left = 40;

        var effective = Assert.Single(sizes);
        Assert.Equal(model.Width, effective.Width);
        Assert.Equal(model.Height, effective.Height);
        Assert.Equal(40, effective.LeftBorder);
        Assert.Equal(model.TopBorder, effective.TopBorder);
    }

    [Fact]
    public void BordersMirrorTheModelWhileTheMonitorDoesNotOwnThem()
    {
        var (policy, model, _) = Build();

        model.LeftBorder = 25;

        Assert.Equal(25, policy.Borders.Left);
        Assert.False(policy.Customized);
    }

    [Fact]
    public void TheMirrorStopsOnceTheMonitorOwnsItsBorders()
    {
        var (policy, model, _) = Build();

        policy.Customized = true;
        model.LeftBorder = 25;

        Assert.Equal(10, policy.Borders.Left);
    }

    [Fact]
    public void AMirrorWriteIsNeverAUserEditEvenWhileAlreadyInPerMonitorMode()
    {
        // The Load path applies model borders with the mode already on PerMonitor. Without the
        // mirror flag that copy would mark the monitor customized and cut the mirror mid-write,
        // freezing the remaining sides at their seeded values.
        var (policy, model, _) = Build(MonitorBorderPolicy.PerMonitor);

        model.LeftBorder = 25;
        model.BottomBorder = 26;

        Assert.False(policy.Customized);
        Assert.Equal(25, policy.Borders.Left);
        Assert.Equal(26, policy.Borders.Bottom);
    }

    [Fact]
    public void EditingABorderInPerMonitorModeMakesTheMonitorOwnThem()
    {
        var (policy, model, _) = Build(MonitorBorderPolicy.PerMonitor);

        policy.Borders.Left = 40;
        Assert.True(policy.Customized);

        // Ownership taken: the model no longer drives this monitor.
        model.LeftBorder = 25;
        Assert.Equal(40, policy.Borders.Left);
    }

    [Fact]
    public void EditingABorderInPerModelModeDoesNotMakeTheMonitorOwnThem()
    {
        // PerModel edits land on the shared model, not here; a write reaching Borders in that
        // mode is the store or a stray caller, and must not silently take ownership.
        var (policy, _, _) = Build();

        policy.Borders.Left = 40;

        Assert.False(policy.Customized);
    }

    [Fact]
    public void CustomizedRaisesAChangeNotificationWhenTheEditIsDetected()
    {
        var (policy, _, _) = Build(MonitorBorderPolicy.PerMonitor);

        var changed = new List<string?>();
        policy.PropertyChanged += (_, e) => changed.Add(e.PropertyName);

        policy.Borders.Left = 40;

        Assert.Contains(nameof(MonitorBorderPolicy.Customized), changed);
    }

    [Fact]
    public void BordersChangedReportsEveryEditPastTheSeedingIncludingMirrorWrites()
    {
        var (policy, model, options) = Build();

        var edits = new List<Unit>();
        using var subscription = policy.BordersChanged.Subscribe(edits.Add);

        // Seeding already happened in the constructor and must not be reported.
        Assert.Empty(edits);

        // A mirror write is not a user edit, but it does change what a save would write out.
        model.LeftBorder = 25;
        Assert.Single(edits);

        options.SetBorderValues(MonitorBorderPolicy.PerMonitor);
        policy.Borders.Top = 30;
        Assert.Equal(2, edits.Count);
    }

    [Fact]
    public void WritingTheSameBorderValueReportsNothing()
    {
        var (policy, _, _) = Build();

        var edits = new List<Unit>();
        using var subscription = policy.BordersChanged.Subscribe(edits.Add);

        policy.Borders.Left = policy.Borders.Left;

        Assert.Empty(edits);
    }

    [Fact]
    public void DisposingStopsTheMirrorAndTheEditReports()
    {
        var (policy, model, _) = Build();

        var edits = new List<Unit>();
        using var subscription = policy.BordersChanged.Subscribe(edits.Add);

        policy.Dispose();
        model.LeftBorder = 25;

        Assert.Equal(10, policy.Borders.Left);
        Assert.Empty(edits);
    }
}
