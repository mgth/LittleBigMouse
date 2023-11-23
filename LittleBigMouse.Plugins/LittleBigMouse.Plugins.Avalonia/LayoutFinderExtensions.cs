using System;
using Avalonia;
using Avalonia.VisualTree;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;

namespace LittleBigMouse.Plugins.Avalonia;

public static class LayoutFinderExtensions
{
    /// <summary>
    /// Retrieve the <see cref="IMonitorsLayoutPresenterView"/> from any children <see cref="Visual"/> 
    /// </summary>
    /// <param name="visual"></param>
    /// <returns></returns>
    public static IMonitorsLayoutPresenterView? GetPresenter(this Visual? visual) 
        => visual.FindAncestorOfType<IMonitorsLayoutPresenterView>();

    /// <summary>
    /// Retrieve layout model from the <see cref="IMonitorsLayoutPresenterView"/> children <see cref="Visual"/>
    /// </summary>
    /// <param name="visual"></param>
    /// <returns></returns>
    public static IMonitorsLayout? GetLayout(this Visual? visual) 
        => visual.GetPresenter()?.ViewModel.Model;

    /// <summary>
    /// get the <see cref="IMonitorFrameView"/> visual parent
    /// </summary>
    /// <param name="visual"></param>
    /// <returns></returns>
    public static IMonitorFrameView? GetMonitorFrame<T>(this T visual) 
        where T : Visual, IMonitorFrameContentViewClass
        => visual.FindAncestorOfType<IMonitorFrameView>();

    /// <summary>
    /// get the <see cref="IFrameLocation"/> from the <see cref="IMonitorFrameView"/> visual parent
    /// </summary>
    /// <param name="visual"></param>
    /// <returns></returns>
    public static IFrameLocation? GetFrameLocation<T>(this T visual) 
        where T : Visual, IMonitorFrameContentViewClass
        => visual.GetMonitorFrame()?.ViewModel?.Location;

    /// <summary>
    /// Assign an <see cref="IFrameLocation"/> to the <see cref="IMonitorFrameView"/> visual parent of the current visual
    /// </summary>
    /// <param name="visual"></param>
    /// <param name="location">The location to be assigned</param>
    /// <returns>The previous <see cref="IFrameLocation"/> </returns>
    /// <exception cref="InvalidOperationException"></exception>
    public static IFrameLocation? SetFrameLocation<T>(this T visual, IFrameLocation location)
        where T : Visual, IMonitorFrameContentViewClass
    {
        if (visual.GetMonitorFrame()?.ViewModel is not { } vm) throw new InvalidOperationException();

        var old = vm.Location;
        vm.Location = location;

        return old;
    }
}