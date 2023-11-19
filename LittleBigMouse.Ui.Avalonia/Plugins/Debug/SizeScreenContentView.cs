using Avalonia.Controls;
using Avalonia.VisualTree;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Debug;

internal class SizeScreenContentView : UserControl, IView<MonitorDebugViewMode, MonitorDebugViewModel>, IViewScreenFrameTopLayer
{
    public MonitorFrameView Frame => this.FindAncestorOfType<MonitorFrameView>();
}