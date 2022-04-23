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

using HLab.ColorTools.Wpf;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Annotations.Icons;
using HLab.Icons.Wpf.Icons;
using HLab.Ioc;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
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
    private T Locate<T>() => Locator<T>.Locate();


    private ResourceDictionary _themeDark;
    private ResourceDictionary _themeLight;

    protected override void OnExit(ExitEventArgs e)
    {
        base.OnExit(e);
    }

    protected override void OnStartup(StartupEventArgs e)
    {

        _themeDark = new ResourceDictionary{Source =  new Uri("/HLab.Base.Wpf;component/Themes/HLab.Theme.Dark.xaml",UriKind.RelativeOrAbsolute)};
        _themeLight = new ResourceDictionary{Source =  new Uri("/HLab.Base.Wpf;component/Themes/HLab.Theme.Light.xaml",UriKind.RelativeOrAbsolute)};

        base.OnStartup(e);

        WatchTheme();

        var parser = new AssemblyParser();
        Locator.Configure();

        SingletonLocator<IEventHandlerService>.Set<EventHandlerServiceWpf>();
        SingletonLocator<IMainService>.Set<MainService>();
        SingletonLocator<IIconService>.Set<IconService>();
        SingletonLocator<IMvvmService>.Set<MvvmServiceWpf>();
        SingletonLocator<IMessageBus>.Set<MessageBus>();

        SingletonLocator<IMonitorsService>.Set<MonitorsService>();

        SingletonLocator<ILittleBigMouseClientService>.Set<LittleBigMouseClientService>();

        Locator<IMonitorsLayout>.Set<Layout>();

        NotifyHelper.EventHandlerService = new EventHandlerServiceWpf();


        parser.LoadDll("LittleBigMouse.Plugin.Location");
        parser.LoadDll("LittleBigMouse.Plugin.Vcp");

        parser.LoadModules();

        parser.Add<IView>(EnumerableLocator<IView>.AddAutoFactory);
        parser.Add<IViewModel>(EnumerableLocator<IViewModel>.AddAutoFactory);

        parser.Add<IBootloader>(EnumerableLocator<IBootloader>.AddAutoFactory);

        parser.Parse();

        Locator.InitSingletons();

        var boot = new Bootstrapper(() => Locate<IEnumerable<IBootloader>>());

        boot.Boot();
    }

    private const string RegistryKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

private const string RegistryValueName = "AppsUseLightTheme";

private enum WindowsTheme
{
	Light,
	Dark
}
private void ApplyResourceDictionaryEntries(ResourceDictionary oldRd, ResourceDictionary newRd)
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

public void WatchTheme()
{
	var currentUser = WindowsIdentity.GetCurrent();
	string query = string.Format(
		CultureInfo.InvariantCulture,
		@"SELECT * FROM RegistryValueChangeEvent WHERE Hive = 'HKEY_USERS' AND KeyPath = '{0}\\{1}' AND ValueName = '{2}'",
		currentUser.User.Value,
		RegistryKeyPath.Replace(@"\", @"\\"),
		RegistryValueName);

	try
	{
		var watcher = new ManagementEventWatcher(query);
		watcher.EventArrived += (sender, args) =>
		{
			SetTheme(GetWindowsTheme());
		};

		// Start listening for events
		watcher.Start();
	}
	catch (Exception)
	{
		// This can fail on Windows 7
	}

	SetTheme(GetWindowsTheme());
}

private void SetTheme(WindowsTheme theme)
    {
        ThemeManager.Current.SyncTheme(ThemeSyncMode.SyncAll);

        switch(theme)
        {
            case WindowsTheme.Light:
                //ThemeManager.Current.ChangeTheme(this, "Light.Blue");
                if(Resources.MergedDictionaries.Contains(_themeDark)) Resources.MergedDictionaries.Remove(_themeDark);
                if(!Resources.MergedDictionaries.Contains(_themeLight))
                    Resources.MergedDictionaries.Add(_themeLight);
                break;

            case WindowsTheme.Dark:
                //ThemeManager.Current.ChangeTheme(this, "Dark.Blue");

                if(Resources.MergedDictionaries.Contains(_themeLight)) Resources.MergedDictionaries.Remove(_themeLight);
                if(!Resources.MergedDictionaries.Contains(_themeDark))
                    Resources.MergedDictionaries.Add(_themeDark);
                break;
        }
    }


private static WindowsTheme GetWindowsTheme()
{
	using (RegistryKey key = Registry.CurrentUser.OpenSubKey(RegistryKeyPath))
	{
		object registryValueObject = key?.GetValue(RegistryValueName);
		if (registryValueObject == null)
		{
			return WindowsTheme.Light;
		}

		int registryValue = (int)registryValueObject;

		return registryValue > 0 ? WindowsTheme.Light : WindowsTheme.Dark;
	}
}

    private static Color GetAccentColor()
    {
	    using var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\DWM");
	
        var value = key.GetValue("AccentColor");

        if(value is string s && int.TryParse(s,out int result))
        {
            return result.ToColor();
        }
        return Colors.Blue;
    }

}

