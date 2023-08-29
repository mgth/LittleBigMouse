using System.Reactive.Linq;
using System.Windows.Input;
using HLab.Mvvm.Annotations;
using ReactiveUI;

namespace LittleBigMouse.Plugins.Avalonia
{
    public static class MainPluginsViewModelExtension
    {
        public static void AddViewModeButton<T>(this IMainPluginsViewModel @this, string id, string iconPath,
            string toolTypeText)
            where T : ViewMode
        {
            var command = ReactiveCommand.Create<bool>(b =>
                {
                    if (b)
                        @this.SetMonitorFrameViewMode<T>();
                    else
                        @this.SetMonitorFrameViewMode<DefaultViewMode>();
                }
                , outputScheduler: RxApp.MainThreadScheduler
                , canExecute: Observable.Return(true));

            @this.AddButton(id,iconPath,toolTypeText,command);
        }

    }
}
