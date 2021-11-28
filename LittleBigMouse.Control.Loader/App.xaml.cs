/*
  LittleBigMouse.Control.Loader
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Loader.

    LittleBigMouse.Control.Loader is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Loader is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Reflection;
using System.Windows;
using HLab.Core;
using HLab.DependencyInjection;
using HLab.Notify.Wpf;

namespace LittleBigMouse.Control.Loader
{
    /// <summary>
    /// Logique d'interaction pour App.xaml
    /// </summary>
    public partial class App : Application
    {
        protected override void OnStartup(StartupEventArgs e) {
            base.OnStartup(e);
            var container = new DependencyInjectionContainer();
            var boot = container.Locate<Bootstrapper>();

            boot.Container.ExportAssembly(Assembly.GetAssembly(typeof(EventHandlerServiceWpf)));

            boot.LoadDll("LittleBigMouse.Control.Core");
            boot.LoadDll("LittleBigMouse.Plugin.Location");
            boot.LoadDll("LittleBigMouse.Plugin.Vcp");

            boot.Boot();
        }
    }

}

