using System.Windows.Input;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The monitor context menu used to carry a "Reset Size" entry over
/// <c>ResetSizesFromSystem</c>, a real command wrapping an empty
/// <c>SetSizesFromSystemConfiguration</c>. Nothing caught it: the binding resolved, the command
/// was assigned and executed without throwing — it just did nothing. Only running the command
/// and looking at the layout afterwards tells the difference, which is what this does for the
/// entries that remain.
/// </summary>
public sealed class ContextMenuCommandTests
{
    static MonitorsLayoutPresenterViewModel PresenterOver(IMonitorsLayout layout)
        => new(new StubMainPluginsViewModel()) { Model = layout };

    [Fact]
    public void ResetLocationsFromSystemMovesMonitorsBackToTheirPixelLayout()
    {
        // Side by side in pixels, but dragged 300 mm apart and 100 mm down in the model.
        // Borders zeroed so contact is plain adjacency rather than adjacency plus two bezels.
        var layout = BorderTestLayouts.TwoMonitors(out var left, out var right);
        foreach (var monitor in new[] { left, right })
        {
            monitor.Model.PhysicalSize.LeftBorder = 0;
            monitor.Model.PhysicalSize.TopBorder = 0;
            monitor.Model.PhysicalSize.RightBorder = 0;
            monitor.Model.PhysicalSize.BottomBorder = 0;
        }

        left.ActiveSource!.Source.Primary = true;
        left.ActiveSource.Source.InPixel.X = 0;
        right.ActiveSource!.Source.InPixel.X = 1920;
        right.DepthProjection.X = 300;
        right.DepthProjection.Y = 100;

        PresenterOver(layout).ResetLocationsFromSystem.Execute(null);

        // Back in contact along the shared edge, and vertically realigned.
        Assert.Equal(left.DepthProjection.X + left.DepthProjection.Width, right.DepthProjection.X, 6);
        Assert.Equal(left.DepthProjection.Y, right.DepthProjection.Y, 6);
    }

    [Fact]
    public void EveryContextMenuCommandIsExecutable()
    {
        var layout = BorderTestLayouts.TwoMonitors(out _, out _);
        var presenter = PresenterOver(layout);

        var commands = typeof(IMonitorsLayoutPresenterViewModel)
            .GetProperties()
            .Where(p => typeof(ICommand).IsAssignableFrom(p.PropertyType))
            .Select(p => (p.Name, Command: (ICommand?)p.GetValue(presenter)))
            .ToList();

        Assert.NotEmpty(commands);
        Assert.All(commands, c =>
        {
            Assert.NotNull(c.Command);
            Assert.True(c.Command!.CanExecute(null), c.Name);
        });
    }

    sealed class StubMainPluginsViewModel : IMainPluginsViewModel
    {
        public ILayoutOptions Options { get; } = new ILayoutOptions.Design();
        public void AddButton(IUiCommand cmd) { }
        public Type ContentViewMode { get; set; } = typeof(DefaultViewMode);
        public Type PresenterViewMode => typeof(DefaultViewMode);
        public IMainService? MainService { get; set; }
    }
}
