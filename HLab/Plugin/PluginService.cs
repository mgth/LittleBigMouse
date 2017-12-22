/*
  HLab.Plugin
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Plugin.

    HLab.Plugin is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Plugin is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using HLab.Mvvm;

namespace HLab.Plugin
{
    public class PluginService : HLab.Base.Singleton<PluginService>
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
            MvvmService.D.Register(assembly);

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
