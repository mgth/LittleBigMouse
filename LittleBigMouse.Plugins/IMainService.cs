using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using HLab.Mvvm.Annotations;
using LittleBigMouse.ScreenConfig;
using LittleBigMouse.ScreenConfig.Dimensions;

namespace LittleBigMouse.Plugins
{
    public interface IMultiScreensViewModel
    {
        IScreenRatio VisualRatio { get; }
    }
    public interface IScreenFrameViewModel : IViewModel<Screen>
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

    public interface IMainService
    {
        void AddButton(ICommand cmd);
        void SetViewMode(Type viewMode);
        void SetViewMode<T>() where T:ViewMode;
    }
}
