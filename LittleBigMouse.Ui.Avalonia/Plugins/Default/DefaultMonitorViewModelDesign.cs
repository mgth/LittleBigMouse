using System;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Default;

public class DefaultMonitorViewModelDesign : DefaultMonitorViewModel, IDesignViewModel
{
    public DefaultMonitorViewModelDesign()
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");

        //Monitor = new MonitorDesign();
    }
}