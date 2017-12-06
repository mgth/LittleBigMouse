using Erp.Base;

namespace Plugin
{
    public interface IPluginModule
    {
        void Register();
    }

    public abstract class PluginModule<T> : Singleton<T>, IPluginModule
        where T : PluginModule<T>
    {
        public abstract void Register();
    }
}
