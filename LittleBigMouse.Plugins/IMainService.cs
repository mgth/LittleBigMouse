using System;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

using HLab.Mvvm.Annotations;

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.Plugins
{
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
        void AddButton(ICommand cmd);
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
}
