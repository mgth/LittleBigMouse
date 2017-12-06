using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Net.Mime;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using Erp.Base;
using ErpSystem;

namespace Plugin
{
    public class PluginService : Erp.Base.Singleton<PluginService>
    {
        private readonly List<IPluginModule> _modules = new List<IPluginModule>();
        public void Register()
        {
            foreach (var f in Directory.GetFiles(AppDomain.CurrentDomain.BaseDirectory, "*.dll"))
            {
                var assembly = Assembly.LoadFile(f);

                var name = Assembly.GetExecutingAssembly().GetName().FullName;

                if (assembly.GetReferencedAssemblies().All(a => a.FullName != name)) continue;

                foreach (var type in assembly.GetTypes())
                {
                    if (!typeof(IPluginModule).IsAssignableFrom(type) || !type.IsClass || type.IsAbstract) continue;

                    var module = (IPluginModule)type.GetProperty("D", BindingFlags.Static | BindingFlags.Public | BindingFlags.FlattenHierarchy)?.GetMethod.Invoke(null,null);
                    _modules.Add(module);
                }
            }

            foreach (var module in _modules)
            {
                module.RegisterIcons();
                PreRegisterPlugin(module);
                module.Register();
            }

        }

        private void PreRegisterPlugin(IPluginModule module)
        {
            var assembly = module.GetType().Assembly;

            var types = assembly.GetTypes();

            foreach (var type in types)
            {
                foreach (var m in type.GetMethods())
                {
                    if (m.GetCustomAttribute<OnRegisterPlugin>() != null)
                    {
                        if (m.IsStatic)
                        {
                            //m.Invoke(null,null);
                            var action = (Action)Delegate.CreateDelegate(typeof(Action), m);
                            action();
                        }
                        else throw new InvalidOperationException("OnRegisterPlugin should be static");
                    }
                }
            }
        }

    }
}
