

using System;
using Avalonia;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.Plugins.Avalonia;

public interface IMultiScreensViewModel
{
    IDisplayRatio VisualRatio { get; }
}
public interface IScreenFrameViewModel : IViewModel<Monitor>
{
    IMultiScreensViewModel Presenter { get; set; }
}

public interface IScreenFrameView
{
    IScreenFrameViewModel ViewModel { get; }
    Thickness Margin { get; set; }
    double ActualHeight { get; }
    double ActualWidth { get; }

}

public interface IMultiScreensView
{
    Panel GetMainPanel();
}

public interface IMainControl
{
    void AddButton(IReactiveCommand cmd);
    void SetViewMode<T>();
}


public interface IMainService
{
    //void AddButton(ICommand cmd);
    //void SetViewMode(Type viewMode);
    //void SetViewMode<T>() where T:ViewMode;
    void StartNotifier();
    void ShowControl();

    void AddControlPlugin(Action<IMainControl> action);
}