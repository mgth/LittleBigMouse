/*
  LittleBigMouse.Control.Loader
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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

using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.Management;
using System.Security.Principal;
using System.Windows;
using System.Windows.Media;

using ControlzEx.Theming;
using Grace.DependencyInjection;
using HLab.Base.Wpf.Themes;
using HLab.ColorTools.Wpf;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Annotations.Icons;
using HLab.Icons.Wpf.Icons;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Wpf;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Notify.Wpf;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.Control.Main;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugins;

using Microsoft.Win32;

namespace LittleBigMouse.Control;

/// <summary>
/// Logique d'interaction pour App.xaml
/// </summary>
public partial class App : Application
{

    protected override void OnExit(ExitEventArgs e)
    {
        base.OnExit(e);
    }

    protected override void OnStartup(StartupEventArgs e)
    {


        base.OnStartup(e);

        //_ = new ThemeWatcher(Resources);

        var container = new DependencyInjectionContainer();
        container.Configure(c =>
        {
            c.Export<EventHandlerServiceWpf>().As<IEventHandlerService>().Lifestyle.Singleton();
            NotifyHelper.EventHandlerService = new EventHandlerServiceWpf();

            c.Export<IconService>().As<IIconService>().Lifestyle.Singleton();
            c.Export<MvvmServiceWpf>().As<IMvvmService>().Lifestyle.Singleton();
            c.Export<MessageBus>().As<IMessagesService>().Lifestyle.Singleton();

            c.Export<MainService>().As<IMainService>().Lifestyle.Singleton();
            c.Export<MonitorsService>().As<IMonitorsService>().Lifestyle.Singleton();
            c.Export<LittleBigMouseClientService>().As<ILittleBigMouseClientService>().Lifestyle.Singleton();

            c.Export<Layout>().As<IMonitorsLayout>();

            var parser = new AssemblyParser();

            parser.LoadDll("LittleBigMouse.Plugin.Location");
            parser.LoadDll("LittleBigMouse.Plugin.Vcp");

            parser.LoadModules();

            parser.Add<IView>(t => c.Export(t).As(typeof(IView)));
            parser.Add<IViewModel>(t => c.Export(t).As(typeof(IViewModel)));
            parser.Add<IBootloader>(t => c.Export(t).As(typeof(IBootloader)));

            parser.Parse();
        });

        var boot = new Bootstrapper(() => container.Locate<IEnumerable<IBootloader>>());

        boot.Boot();
    }


    void ApplyResourceDictionaryEntries(ResourceDictionary oldRd, ResourceDictionary newRd)
    {
        foreach (ResourceDictionary mergedDictionary in newRd.MergedDictionaries)
        {
            ApplyResourceDictionaryEntries(oldRd, mergedDictionary);
        }

        foreach (DictionaryEntry item in newRd)
        {
            if (oldRd.Contains(item.Key))
            {
                oldRd.Remove(item.Key);
            }

            oldRd.Add(item.Key, item.Value);
        }
    }






}

