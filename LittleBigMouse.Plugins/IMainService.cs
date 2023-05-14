namespace LittleBigMouse.Plugins
{
    public interface IMainService
    {
        //void AddButton(ICommand cmd);
        //void SetViewMode(Type viewMode);
        //void SetViewMode<T>() where T:ViewMode;
        void StartNotifier();
        Task ShowControlAsync();

        void AddControlPlugin(Action<IMainPluginsViewModel>? action);
    }
}
