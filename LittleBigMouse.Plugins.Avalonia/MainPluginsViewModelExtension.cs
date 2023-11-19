using System;
using System.Reactive.Linq;
using HLab.Mvvm.Annotations;
using ReactiveUI;

namespace LittleBigMouse.Plugins.Avalonia;

public static class MainPluginsViewModelExtension
{
    public static void AddViewModeButton<T>(this IMainPluginsViewModel @this, string id, string iconPath,
        string toolTypeText)
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
            , outputScheduler: RxApp.MainThreadScheduler
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

        @this.AddButton(command);
    }

}