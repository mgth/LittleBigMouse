using System;
using System.Reactive.Linq;
using HLab.Mvvm.Annotations;
using ReactiveUI;

namespace LittleBigMouse.Plugins.Avalonia;

public static class MainPluginsViewModelExtension
{
    /// <param name="isVisible">Optional visibility source for the button (e.g. an
    /// option toggle). When it turns false while this view mode is active, the
    /// presenter falls back to the default view mode.</param>
    public static void AddViewModeButton<T>(this IMainPluginsViewModel @this, string id, string iconPath,
        string toolTypeText, IObservable<bool>? isVisible = null)
        where T : ViewMode
    {
        var rc = ReactiveCommand.Create<bool>(b =>
            {
                if (b)
                    @this.SetMonitorFrameViewMode<T>();
                else
                {
                    if(@this.ContentViewMode==typeof(T))
                        @this.SetMonitorFrameViewMode<DefaultViewMode>();
                }
            }
            , outputScheduler: RxSchedulers.MainThreadScheduler
            , canExecute: Observable.Return(true));


        var command = new UiCommand(id)
        {
            Command = rc,
            IconPath = iconPath,
            ToolTipText = toolTypeText,
        };

        @this.WhenAnyValue(e => e.ContentViewMode).Do(e =>
        {
            if (e == typeof(T)) return;
            command.IsActive = false;
        }).Subscribe(Console.WriteLine);

        isVisible?
            .ObserveOn(RxSchedulers.MainThreadScheduler)
            .Subscribe(visible =>
            {
                command.IsVisible = visible;
                if (!visible && @this.ContentViewMode == typeof(T))
                    @this.SetMonitorFrameViewMode<DefaultViewMode>();
            });

        @this.AddButton(command);
    }

}