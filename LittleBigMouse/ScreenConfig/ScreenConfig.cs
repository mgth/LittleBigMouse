/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.IO;
using System.Linq;
using System.Management;
using System.Windows;
using HLab.Base;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Windows.API;
using HLab.Windows.Monitors;
using Microsoft.Win32;
using Newtonsoft.Json;

// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenConfig : N<ScreenConfig>
    {
        [Import]
        public ScreenConfig(IMonitorsService monitorsService)
        {
            MonitorsService = monitorsService;

            Initialize();

            _wallPaperPath.Set(GetCurrentDesktopWallpaper());

            SystemEvents.UserPreferenceChanged += SystemEvents_UserPreferenceChanged;

            MonitorsOnCollectionChanged(monitorsService.AttachedMonitors,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, monitorsService.AttachedMonitors.ToList()));

            monitorsService.AttachedMonitors.CollectionChanged += MonitorsOnCollectionChanged;
        }

        internal IMonitorsService MonitorsService { get; }

        private readonly IProperty<string> _id = H.Property<string>(c => c
            .On(e => e.AllScreens.Item().Id)
            .Set(e => e.AllScreens.OrderBy(s => s.Id)
                .Aggregate("", (current, screen) => current + (current != "" ? "." : "") + screen.Id)));


        [JsonProperty]
        public string Id => _id.Get();
        public static RegistryKey OpenRootRegKey(bool create = false)
        {
            using (RegistryKey key = Registry.CurrentUser)
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(ROOT_KEY) : key.OpenSubKey(ROOT_KEY);
            }
        }

        /// <returns>a list of string representing each known config in registry</returns>
        /// <summary>
        /// 
        /// </summary>
        public static IEnumerable<string> ConfigsList
        {
            get
            {
                using (var rootkey = OpenRootRegKey())
                {
                    using (var key = rootkey.OpenSubKey("configs"))
                    {
                        return key?.GetSubKeyNames();
                    }
                }
            }
        }

        public Screen ScreenFromPixel(Point pixel)
        {
            foreach (var screen in AllScreens)
            {
                if (screen.InPixel.Bounds.Contains(pixel)) return screen;
            }

            return null;
        }
        public Screen ScreenFromMmPosition(Point mm)
        {
            foreach (var screen in AllScreens)
            {
                if (screen.InMm.Bounds.Contains(mm)) return screen;
            }

            return null;
        }




        private void SystemEvents_UserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
        {
            _wallPaperPath.Set(GetCurrentDesktopWallpaper());
        }

        public string WallPaperPath => _wallPaperPath.Get();
        private readonly IProperty<string> _wallPaperPath = H.Property<string>();

        public string GetCurrentDesktopWallpaper()
        {
            string currentWallpaper = new string('\0', NativeMethods.MAX_PATH);
            NativeMethods.SystemParametersInfo(NativeMethods.SPI_GETDESKWALLPAPER, currentWallpaper.Length, currentWallpaper, 0);
            return currentWallpaper.Substring(0, currentWallpaper.IndexOf('\0'));
        }

        public bool TiledWallpaper => _tiledWallpaper.Get();
        private readonly IProperty<bool> _tiledWallpaper = H.Property<bool>(c => c
                 .Set(s =>
                {
                    using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
                    {
                        if (key == null) return false;
                        return key.GetValue("WallpaperStyle", "0").ToString() == "1";
                    }
                }));


        public int WallpaperStyle => _wallpaperStyle.Get();
        private readonly IProperty<int> _wallpaperStyle = H.Property<int>(nameof(WallpaperStyle), c => c
                 .Set(s =>
                {
                    using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
                    {
                        if (key == null) return 0;

                        if (int.TryParse(key.GetValue("WallpaperStyle", "0").ToString(), out var value))
                        {
                            return value;
                        }
                        return 0;
                    }
                }));

        private readonly IProperty<int[]> _backgroundColor = H.Property<int[]>(nameof(BackGroundColor), c => c
             .Set(e =>
                {
                    using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Colors", false))
                    {
                        if (key == null) return new[] { 0, 0, 0 };
                        var s = key.GetValue("Background", "0 0 0").ToString();
                        var ss = s.Split(' ');
                        var i = ss.Select(int.Parse).ToArray();
                        return i;
                    }
                }
            ));
        public int[] BackGroundColor => _backgroundColor.Get();

        private void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            switch (args.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    if (args.NewItems != null)
                        foreach (var monitor in args.NewItems.OfType<Monitor>())
                        {
                            var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));
                            if (screen != null) continue;

                            screen = new Screen(this, monitor);
                            AllScreens.Add(screen);
                        }
                    break;

                case NotifyCollectionChangedAction.Remove:
                    if (args.OldItems != null)
                        foreach (var monitor in args.OldItems.OfType<Monitor>())
                        {
                            var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));

                            if (screen != null) AllScreens.Remove(screen);
                        }
                    break;

                case NotifyCollectionChangedAction.Replace:
                case NotifyCollectionChangedAction.Move:
                case NotifyCollectionChangedAction.Reset:
                    throw new NotImplementedException();
                default:
                    throw new ArgumentOutOfRangeException();
            }


            Load();
            SetPhysicalAuto(false);
        }

        [JsonProperty]
        public ObservableCollectionSafe<Screen> AllScreens { get; } = new ObservableCollectionSafe<Screen>();

        public IEnumerable<Screen> AllBut(Screen screen) => AllScreens.Where(s => !Equals(s, screen));

        public Screen Selected => _selected.Get();
        private readonly IProperty<Screen> _selected = H.Property<Screen>(c => c
            .On(e => e.AllScreens.Item().Selected)
            .Set(e => e.AllScreens.FirstOrDefault(screen => screen.Selected))
        );

        private const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

        internal static RegistryKey OpenConfigRegKey(string configId, bool create)
        {
            using (var key = OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"configs\" + configId) : key.OpenSubKey(@"configs\" + configId);
            }
        }

        internal static string ConfigPath(string configId, bool create)
        {
            var path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse", configId);

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        public string ConfigPath(bool create) => ConfigPath(Id, create);

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Id, create);



        public void MatchConfig(string id)
        {
            using (var rootKey = OpenRootRegKey())
            {
                using (var key = rootKey.OpenSubKey(@"configs\" + id))
                {
                    var todo = key.GetSubKeyNames().ToList();

                    foreach (var screen in AllScreens)
                    {
                        if (todo.Contains(screen.Monitor.IdMonitor))
                        {
                            AttachToDesktop(id, screen.Monitor.IdMonitor, false);
                            todo.Remove(screen.Monitor.IdMonitor);
                        }
                        else
                        {
                            MonitorsService.DetachFromDesktop(screen.Monitor.AttachedDisplay.DeviceName, false);
                        }
                    }

                    foreach (string s in todo)
                    {
                        AttachToDesktop(id, s, false);
                    }

                    MonitorsService.ApplyDesktop();
                }
            }
        }

        public bool IsDoableConfig(String id)
        {
            using (var rootKey = OpenRootRegKey())
            {
                using (var key = rootKey.OpenSubKey(@"configs\" + id))
                {
                    var todo = key.GetSubKeyNames().ToList();

                    foreach (var s in todo)
                    {
                        var m = MonitorsService.Monitors.FirstOrDefault(
                            d => s == d.IdMonitor);

                        if (m == null) return false;
                    }
                }
            }
            return true;
        }

        public void AttachToDesktop(string configId, string monitorId, bool apply = true)
        {
            //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
            //{
            //    id = monkey?.GetValue("DeviceId").ToString();
            //    if (id == null) return;
            //}
            var area = new Rect();
            var primary = false;
            var orientation = 0;

            using (var monkey = Screen.OpenConfigRegKey(configId, monitorId))
            {
                area.X = double.Parse(monkey.GetValue("PixelX").ToString());
                area.Y = double.Parse(monkey.GetValue("PixelY").ToString());
                area.Width = double.Parse(monkey.GetValue("PixelWidth").ToString());
                area.Height = double.Parse(monkey.GetValue("PixelHeight").ToString());

                primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
                orientation = (int)double.Parse(monkey.GetValue("Orientation").ToString());
            }

            var monitor = MonitorsService.Monitors.FirstOrDefault(
                d => monitorId == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

            if (monitor != null)
                MonitorsService.AttachToDesktop(monitor.AttachedDisplay.DeviceName, primary, area, orientation, apply);
        }

        public void EnumWmi()
        {
            const string namespacePath = "\\\\.\\ROOT\\WMI\\ms_409";
            const string className = "WmiMonitorID";

            //Create ManagementClass
            var oClass = new ManagementClass(namespacePath + ":" + className);

            //Get all instances of the class and enumerate them
            foreach (var o in oClass.GetInstances().OfType<ManagementObject>())
            {
                //access a property of the Management object
                Console.WriteLine("ManufacturerName : {0}", o["ManufacturerName"]);
            }
        }



        public void Load()
        {
            using (var k = OpenConfigRegKey())
            {
                if (k != null)
                {
                    Enabled = k.GetValue("Enabled", 0).ToString() == "1";
                    AdjustPointer = k.GetValue("AdjustPointer", 0).ToString() == "1";
                    AdjustSpeed = k.GetValue("AdjustSpeed", 0).ToString() == "1";
                    AllowCornerCrossing = k.GetValue("AllowCornerCrossing", 0).ToString() == "1";
                    AllowOverlaps = k.GetValue("AllowOverlaps", 0).ToString() == "1";
                    AllowDiscontinuity = k.GetValue("AllowDiscontinuity", 0).ToString() == "1";
                    LoadAtStartup = k.GetValue("LoadAtStartup", 0).ToString() == "1";
                    HomeCinema = k.GetValue("HomeCinema", 0).ToString() == "1";
                    Pinned = k.GetValue("Pinned", 0).ToString() == "1";
                    LoopX = k.GetValue("LoopX", 0).ToString() == "1";
                    LoopY = k.GetValue("LoopY", 0).ToString() == "1";
                    AutoUpdate = k.GetValue("AutoUpdate", 0).ToString() == "1";
                }
            }

            foreach (Screen s in AllScreens)
            {
                s.Load();
            }
            Saved = true;
        }

        public bool Save()
        {
            using (RegistryKey k = OpenConfigRegKey(true))
            {
                if (k != null)
                {
                    k.SetValue("Enabled", Enabled ? "1" : "0");
                    k.SetValue("AdjustPointer", AdjustPointer ? "1" : "0");
                    k.SetValue("AdjustSpeed", AdjustSpeed ? "1" : "0");
                    k.SetValue("AllowCornerCrossing", AllowCornerCrossing ? "1" : "0");
                    k.SetValue("AllowOverlaps", AllowOverlaps ? "1" : "0");
                    k.SetValue("AllowDiscontinuity", AllowDiscontinuity ? "1" : "0");
                    k.SetValue("LoadAtStartup", LoadAtStartup ? "1" : "0");
                    k.SetValue("HomeCinema", HomeCinema ? "1" : "0");
                    k.SetValue("Pinned", Pinned ? "1" : "0");
                    k.SetValue("LoopX", LoopX ? "1" : "0");
                    k.SetValue("LoopY", LoopY ? "1" : "0");
                    k.SetValue("AutoUpdate", AutoUpdate ? "1" : "0");

                    foreach (Screen s in AllScreens)
                        s.Save(k);

                    Saved = true;
                    return true;
                }
                return false;
            }
        }

        private readonly IProperty<bool> _autoUpdate = H.Property<bool>(nameof(AutoUpdate));
        [JsonProperty]
        public bool AutoUpdate
        {
            get => _autoUpdate.Get();
            set
            {
                if (_autoUpdate.Set(value)) Saved = false;
            }
        }

        [JsonProperty]
        public Screen PrimaryScreen => AllScreens.FirstOrDefault(s => s.Primary);

        [JsonProperty]
        public Rect PhysicalOutsideBounds => _physicalOutsideBounds.Get();
        private readonly IProperty<Rect> _physicalOutsideBounds = H.Property<Rect>(nameof(PhysicalOutsideBounds), c => c

            .On(nameof(AllScreens), "Item", "InMm", "OutsideBounds")
           .Set(e =>
               {
                   var outside = new Rect();

                   var first = true;
                   foreach (var s in e.AllScreens)
                   {
                       if (first)
                       {
                           outside = s.InMm.OutsideBounds;
                           first = false;
                           continue;
                       }

                       outside.Union(s.InMm.OutsideBounds);
                   }

                   return outside;
               }
           ));



        /// <summary>
        /// Mm Bounds of overall screens without borders
        /// </summary>
        [JsonProperty]
        public Rect PhysicalBounds => _physicalBounds.Get();
        private readonly IProperty<Rect> _physicalBounds = H.Property<Rect>(nameof(PhysicalBounds), c => c
           .On(nameof(AllScreens), "Item", "InMm", "Bounds")
           .Set(e =>
               {
                   var inside = new Rect();

                   var first = true;
                   foreach (var s in e.AllScreens)
                   {
                       if (first)
                       {
                           inside = s.InMm.Bounds;
                           first = false;
                           continue;
                       }

                       inside.Union(s.InMm.Bounds);
                   }

                   return inside;
               }
           ));



        /// <summary>
        /// 
        /// </summary>

        [JsonProperty]
        public bool Enabled
        {
            get => _enabled.Get();
            set { if (_enabled.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _enabled = H.Property<bool>(nameof(Enabled));

        [JsonProperty]
        public bool LoadAtStartup
        {
            get => _loadAtStartup.Get();
            set { if (_loadAtStartup.Set(value)) { Saved = false; } }
        }
        private readonly IProperty<bool> _loadAtStartup = H.Property<bool>(nameof(LoadAtStartup));

        [JsonProperty]
        public bool LoopAllowed => true;


        [JsonProperty]
        public bool LoopX
        {
            get => _loopX.Get() && LoopAllowed;
            set { if (_loopX.Set(value)) { Saved = false; } }
        }
        private readonly IProperty<bool> _loopX = H.Property<bool>(nameof(LoopX), c => c
            .On(nameof(LoopAllowed)));


        [JsonProperty]
        public bool LoopY
        {
            get => LoopAllowed && _loopY.Get();
            set { if (_loopY.Set(value)) { Saved = false; } }
        }
        private readonly IProperty<bool> _loopY = H.Property<bool>(nameof(LoopY), c => c
             .On(nameof(LoopAllowed)));


        [JsonProperty]
        public bool IsRatio100 => _isRation100.Get();
        private readonly IProperty<bool> _isRation100 = H.Property<bool>(nameof(IsRatio100), c => c
                 //.TriggerOn(c => c.AllScreens.Item().PixelToDipRatio)
                 .On(nameof(AllScreens), "Item", "PixelToDipRatio")
                .Set(e => e._getIsRatio100()))
            ;
        //[TriggerOn(nameof(AllScreens),"Item","PixelToDipRatio")]
        private bool _getIsRatio100()
        {
            foreach (Screen screen in AllScreens)
            {
                if (screen.PixelToDipRatio.X != 1) return false;
                if (screen.PixelToDipRatio.Y != 1) return false;
            }
            return true;
        }


        [JsonProperty]
        [TriggerOn(nameof(IsRatio100))]
        public bool AdjustPointerAllowed => IsRatio100;


        [JsonProperty]
        public bool AdjustPointer
        {
            get => AdjustPointerAllowed && _adjustPointer.Get();
            set { if (_adjustPointer.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _adjustPointer = H.Property<bool>(nameof(AdjustPointer));

        [JsonProperty]
        [TriggerOn(nameof(IsRatio100))]
        public bool AdjustSpeedAllowed => IsRatio100;

        [JsonProperty]
        public bool AdjustSpeed
        {
            get => AdjustSpeedAllowed && _adjustSpeed.Get();
            set { if (_adjustSpeed.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _adjustSpeed = H.Property<bool>(nameof(AdjustSpeed));

        [JsonProperty]
        public bool AllowCornerCrossing
        {
            get => _allowCornerCrossing.Get();
            set { if (_allowCornerCrossing.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _allowCornerCrossing = H.Property<bool>(nameof(AllowCornerCrossing));

        [JsonProperty]
        public bool HomeCinema
        {
            get => _homeCinema.Get();
            set { if (_homeCinema.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _homeCinema = H.Property<bool>(nameof(HomeCinema));

        [JsonProperty]
        public bool Pinned
        {
            get => _pinned.Get();
            set { if (_pinned.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _pinned = H.Property<bool>(nameof(Pinned));

        [JsonProperty]
        public Rect ConfigLocation
        {
            get => _configLocation.Get();
            set => _configLocation.Set(value);
        }
        private readonly IProperty<Rect> _configLocation = H.Property<Rect>(nameof(ConfigLocation));


        public void SetPhysicalAuto(bool placeall = true)
        {
            if (PrimaryScreen == null) return;

            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }
            // List all screens not positioned
            List<Screen> unatachedScreens = placeall ? AllScreens.ToList() : AllScreens.Where(s => !s.Placed).ToList();

            // start with primary screen
            Queue<Screen> todo = new Queue<Screen>();
            todo.Enqueue(PrimaryScreen);

            while (todo.Count > 0)
            {
                foreach (Screen s2 in todo)
                {
                    unatachedScreens.Remove(s2);
                }

                Screen placedScreen = todo.Dequeue();

                foreach (Screen screenToPlace in unatachedScreens)
                {
                    if (screenToPlace == placedScreen) continue;

                    bool done = false;

                    //     __
                    //  __| A
                    // B  |__
                    //  __|
                    if (screenToPlace.InPixel.Bounds.X == placedScreen.InPixel.Bounds.Right)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Right + screenToPlace.InMm.LeftBorder;
                        done = true;
                    }
                    //B |___|_
                    //A  |    |
                    if (screenToPlace.InPixel.Bounds.Y == placedScreen.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Bottom + screenToPlace.InMm.TopBorder;
                        done = true;
                    }

                    //     __
                    //  __| B
                    // A  |__
                    //  __|
                    if (screenToPlace.InPixel.Bounds.Right == placedScreen.InPixel.Bounds.X)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Left -
                                                  screenToPlace.InMm.OutsideBounds.Width + screenToPlace.InMm.LeftBorder;
                        done = true;
                    }

                    //A |___|_
                    //B  |    |

                    if (screenToPlace.InPixel.Bounds.Bottom == placedScreen.InPixel.Y)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Top -
                                                  screenToPlace.InMm.OutsideBounds.Height + screenToPlace.InMm.TopBorder;
                        done = true;
                    }


                    //  __
                    // |
                    // |__
                    //  __
                    // |
                    // |__
                    if (screenToPlace.InPixel.Bounds.X == placedScreen.InPixel.Bounds.X)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.X;
                        done = true;
                    }

                    //  ___   ___
                    // |   | |   |
                    if (screenToPlace.InPixel.Bounds.Y == placedScreen.InPixel.Bounds.Y)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.Y;
                        done = true;
                    }

                    // __
                    //   |
                    // __|
                    // __
                    //   |
                    // __|
                    if (screenToPlace.InPixel.Bounds.Right == placedScreen.InPixel.Bounds.Right)
                    {
                        screenToPlace.InMm.X = placedScreen.InMm.Bounds.Right - screenToPlace.InMm.Bounds.Width;
                        done = true;
                    }

                    //|___||___|
                    if (screenToPlace.InPixel.Bounds.Bottom == placedScreen.InPixel.Bounds.Bottom)
                    {
                        screenToPlace.InMm.Y = placedScreen.InMm.Bounds.Bottom -
                                                  screenToPlace.InMm.Bounds.Height;
                        done = true;
                    }
                    if (done)
                    {
                        todo.Enqueue(screenToPlace);
                    }
                }

                lock (_compactLock)
                {
                    _compacting = false;
                }
            }
        }

        private readonly object _compactLock = new object();
        private bool _compacting;

        public void Compact()
        {
            //            return;
            if (PrimaryScreen == null) return;


            //if (Moving) return;
            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }

            List<Screen> done = new List<Screen> { PrimaryScreen };

            List<Screen> todo = AllBut(PrimaryScreen).OrderBy(s => s.Distance(PrimaryScreen)).ToList();

            while (todo.Count > 0)
            {
                Screen screen = todo[0];
                todo.Remove(screen);

                screen.PlaceAuto(done);
                done.Add(screen);

                todo = todo.OrderBy(s => s.Distance(done)).ToList();
            }

            lock (_compactLock)
            {
                _compacting = false;
            }
        }

        [JsonProperty]
        public bool AllowOverlaps
        {
            get => _allowOverlaps.Get();
            set
            {
                if (_allowOverlaps.Set(value))
                {
                    Saved = false;
                }
            }
        }
        private readonly IProperty<bool> _allowOverlaps = H.Property<bool>(nameof(AllowOverlaps));

        [JsonProperty]
        public bool AllowDiscontinuity
        {
            get => _allowDiscontinuity.Get();
            set { if (_allowDiscontinuity.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _allowDiscontinuity = H.Property<bool>(nameof(AllowDiscontinuity));


        public bool Saved
        {
            get => _saved.Get();
            set => _saved.Set(value);
        }
        private readonly IProperty<bool> _saved = H.Property<bool>(nameof(Saved));

        [JsonProperty]
        public double MaxEffectiveDpiX => _maxEffectiveDpiX.Get();
        private readonly IProperty<double> _maxEffectiveDpiX = H.Property<double>(c => c
            .On(e => e.AllScreens.Item().EffectiveDpi.X)
            .Set(e => e.AllScreens.Count == 0 ? 0 : e.AllScreens.Select(screen => screen.EffectiveDpi.X).Max())
        );

        [JsonProperty]
        public double MaxEffectiveDpiY => _maxEffectiveDpiY.Get();
        private readonly IProperty<double> _maxEffectiveDpiY = H.Property<double>(c => c
            .On(e => e.AllScreens.Item().EffectiveDpi.Y)
            .Set(e => e.AllScreens.Count == 0 ? 0 : e.AllScreens.Select(screen => screen.EffectiveDpi.Y).Max())
        );


        public ConcurrentDictionary<string, ScreenModel> ScreenModels = new ConcurrentDictionary<string, ScreenModel>();

        public ScreenModel GetScreenModel(string pnpCode, Monitor monitor)
        {
            return ScreenModels.GetOrAdd(pnpCode, s => new ScreenModel(s, this).Load(monitor));
        }
    }
}
