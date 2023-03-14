namespace LittleBigMouse.Plugins
{
    public interface IMainService
    {
        //void AddButton(ICommand cmd);
        //void SetViewMode(Type viewMode);
        //void SetViewMode<T>() where T:ViewMode;
        void StartNotifier();
        void ShowControl();

        void AddControlPlugin(Action<IMainPluginsViewModel>? action);
    }
}
