/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

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
using System.Collections.Specialized;
using System.IO;
using System.Linq;
using System.Management;
using System.Runtime.Serialization;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Media;

using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.Zoning;

using Microsoft.Win32;
using Microsoft.Win32.TaskScheduler;

// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LittleBigMouse.DisplayLayout
{
    using H = H<Layout>;

    class SystemEventProxy
    {
        public event UserPreferenceChangedEventHandler UserPreferenceChanged;

        public SystemEventProxy()
        {
            SystemEvents.UserPreferenceChanged += (sender, args) => UserPreferenceChanged?.Invoke(sender, args);
        }
    }

    public interface IMonitorsLayout
    {
        bool Saved { get; }
        Rect InMmOutsideBounds { get; }
        ObservableCollectionSafe<Monitor> AllMonitors { get; }
        ZonesLayout ComputeZones();
        bool Save();
    }

    [DataContract]
    public class Layout : NotifierBase, IMonitorsLayout
    {
        private static readonly SystemEventProxy SystemEventProxy = new SystemEventProxy();

        public Layout(IMonitorsService monitorsService)
        {
            MonitorsService = monitorsService;

            H.Initialize(this);

            _wallPaperPath.Set(GetCurrentDesktopWallpaper());

            NotifyHelper.EventHandlerService.AddHandler<SystemEventProxy, UserPreferenceChangedEventArgs>(SystemEventProxy, "UserPreferenceChanged", SystemEvents_UserPreferenceChanged);

            MonitorsOnCollectionChanged(monitorsService.AttachedMonitors,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, monitorsService.AttachedMonitors.ToList()));

            NotifyHelper.EventHandlerService.AddHandler(monitorsService.AttachedMonitors, MonitorsOnCollectionChanged);
        }

        internal IMonitorsService MonitorsService { get; }
        public NativeMethods.DPI_Awareness_Context DpiAwarenessContext => _dpiAwarenessContext.Get();
        private readonly IProperty<NativeMethods.DPI_Awareness_Context> _dpiAwarenessContext
            = H.Property<NativeMethods.DPI_Awareness_Context>(c => c
                 .Set(s => NativeMethods.GetThreadDpiAwarenessContext())
            );


        [DataMember]
        public string Id => _id.Get();
        private readonly IProperty<string> _id = H.Property<string>(c => c
            .Set(e => e.AllMonitors.OrderBy(s => s.Id)
                .Aggregate("", (current, screen) => current + (current != "" ? "." : "") + screen.Id))
            .On(e => e.AllMonitors.Item().Id)
            .Update()
        );

        public static RegistryKey OpenRootRegKey(bool create = false)
        {
            using var key = Registry.CurrentUser;
            return create ? key.CreateSubKey(ROOT_KEY) : key.OpenSubKey(ROOT_KEY);
        }

        /// <returns>a list of string representing each known config in registry</returns>
        /// <summary>
        /// 
        /// </summary>
        public static IEnumerable<string> LayoutsList
        {
            get
            {
                using var rootKey = OpenRootRegKey();
                using var key = rootKey?.OpenSubKey("Layouts");
                if (key == null) return new List<string>();
                return key?.GetSubKeyNames();
            }
        }

        public MonitorSource MonitorSourceFromPixel(Point pixel)
        {
            foreach (var source in AllSources)
            {
                if (source.InPixel.Bounds.Contains(pixel)) return source;
            }

            return null;
        }

        public Monitor MonitorFromPhysicalPosition(Point mm)
        {
            foreach (var screen in AllMonitors)
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
        private readonly IProperty<int> _wallpaperStyle = H.Property<int>(c => c
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

        public int[] BackgroundColor => _backgroundColor.Get();
        private readonly IProperty<int[]> _backgroundColor = H.Property<int[]>(c => c
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

        private void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            switch (args.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    if (args.NewItems != null)
                        foreach (var device in args.NewItems.OfType<MonitorDevice>())
                        {
                            var source = AllSources.FirstOrDefault(s => s.Device.Equals(device));
                            if (source != null) continue;

                            var monitor = AllMonitors.FirstOrDefault(s =>
                            {
                                return s.Model.PnpCode == device.PnpCode && s.ActiveSource.Device.IdPhysicalMonitor == device.IdPhysicalMonitor;
                            });
                            if (monitor == null)
                            {
                                monitor = new Monitor(this, device);
                                source = monitor.ActiveSource;

                                AllMonitors.Add(monitor);
                            }
                            else
                            {
                                source = new MonitorSource(monitor, device);
                                monitor.Sources.Add(source);
                            }

                            AllSources.Add(source);
                        }
                    break;

                case NotifyCollectionChangedAction.Remove:
                    if (args.OldItems != null)
                        foreach (var monitor in args.OldItems.OfType<MonitorDevice>())
                        {
                            var screen = AllSources.FirstOrDefault(s => s.Monitor.Equals(monitor));

                            if (screen != null) AllSources.Remove(screen);
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

        [DataMember]
        public ObservableCollectionSafe<Monitor> AllMonitors { get; } = new ObservableCollectionSafe<Monitor>();
        public ObservableCollectionSafe<MonitorSource> AllSources { get; } = new ObservableCollectionSafe<MonitorSource>();

        public IEnumerable<Monitor> AllBut(Monitor screen) => AllMonitors.Where(s => !Equals(s, screen));

        public Monitor Selected => _selected.Get();
        private readonly IProperty<Monitor> _selected = H.Property<Monitor>(c => c
            .On(e => e.AllMonitors.Item().Selected)
            .Set(e => e.AllMonitors.FirstOrDefault(screen => screen.Selected))
        );

        private const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

        //internal static RegistryKey OpenLayoutRegKey(string configId, bool create)
        //{
        //    using (var key = OpenRootRegKey(create))
        //    {
        //        if (key == null) return null;
        //        return create ? key.CreateSubKey(@"configs\" + configId) : key.OpenSubKey(@"configs\" + configId);
        //    }
        //}

        internal static string LayoutPath(string layoutId, bool create)
        {
            var path = Path.Combine(Environment.GetFolderPath(
                Environment.SpecialFolder.LocalApplicationData), "LittleBigMouse", layoutId);

            if (create) Directory.CreateDirectory(path);

            return path;
        }

        public string LayoutPath(bool create) => LayoutPath(Id, create);


        public static RegistryKey OpenRegKey(string layoutId, bool create = false)
        {
            using var key = OpenRootRegKey(create);

            if (key == null) return null;
            return create ? key.CreateSubKey(@"Layouts\" + layoutId) : key.OpenSubKey(@"Layouts\" + layoutId);
        }
        public RegistryKey OpenRegKey(bool create = false) => OpenRegKey(Id, create);



        public void MatchLayout(string id)
        {
            using (var rootKey = OpenRootRegKey())
            {
                using (var key = rootKey.OpenSubKey(@"Layouts\" + id))
                {
                    var todo = key.GetSubKeyNames().ToList();

                    foreach (var source in AllSources)
                    {
                        if (todo.Contains(source.Device.IdMonitor))
                        {
                            AttachToDesktop(id, source.Device.IdMonitor, false);
                            todo.Remove(source.Device.IdMonitor);
                        }
                        else
                        {
                            MonitorsService.DetachFromDesktop(source.Device.AttachedDisplay.DeviceName, false);
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

        public bool IsDoableLayout(string id)
        {
            using (var rootKey = OpenRootRegKey())
            {
                using (var key = rootKey.OpenSubKey(@"Layouts\" + id))
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

        //TODO : wong : should support sources
        public void AttachToDesktop(string layoutId, string monitorId, bool apply = true)
        {
            //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
            //{
            //    id = monkey?.GetValue("DeviceId").ToString();
            //    if (id == null) return;
            //}
            var area = new Rect();
            var primary = false;
            var orientation = 0;

            using (var monkey = Monitor.OpenRegKey(layoutId, monitorId))
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
            using (var key = OpenRegKey())
            {
                if (key != null)
                {
                    Enabled = key.GetValue("Enabled", 0).ToString() == "1";
                    AdjustPointer = key.GetValue("AdjustPointer", 0).ToString() == "1";
                    AdjustSpeed = key.GetValue("AdjustSpeed", 0).ToString() == "1";
                    AllowCornerCrossing = key.GetValue("AllowCornerCrossing", 0).ToString() == "1";
                    AllowOverlaps = key.GetValue("AllowOverlaps", 0).ToString() == "1";
                    AllowDiscontinuity = key.GetValue("AllowDiscontinuity", 0).ToString() == "1";
                    HomeCinema = key.GetValue("HomeCinema", 0).ToString() == "1";
                    Pinned = key.GetValue("Pinned", 0).ToString() == "1";
                    LoopX = key.GetValue("LoopX", 0).ToString() == "1";
                    LoopY = key.GetValue("LoopY", 0).ToString() == "1";
                    AutoUpdate = key.GetValue("AutoUpdate", 0).ToString() == "1";
                }

                LoadAtStartup = IsScheduled();

                if (key != null)
                {
                    foreach (Monitor s in AllMonitors)
                    {
                        s.Load(key);
                    }
                }
            }
            Saved = true;
        }

        public bool Save()
        {
            using (RegistryKey k = OpenRegKey(true))
            {
                if (k != null)
                {
                    k.SetValue("Enabled", Enabled ? "1" : "0");
                    k.SetValue("AdjustPointer", AdjustPointer ? "1" : "0");
                    k.SetValue("AdjustSpeed", AdjustSpeed ? "1" : "0");
                    k.SetValue("AllowCornerCrossing", AllowCornerCrossing ? "1" : "0");
                    k.SetValue("AllowOverlaps", AllowOverlaps ? "1" : "0");
                    k.SetValue("AllowDiscontinuity", AllowDiscontinuity ? "1" : "0");
                    k.SetValue("HomeCinema", HomeCinema ? "1" : "0");
                    k.SetValue("Pinned", Pinned ? "1" : "0");
                    k.SetValue("LoopX", LoopX ? "1" : "0");
                    k.SetValue("LoopY", LoopY ? "1" : "0");
                    k.SetValue("AutoUpdate", AutoUpdate ? "1" : "0");

                    if (LoadAtStartup) Schedule(); else Unschedule();

                    foreach (Monitor s in AllMonitors)
                        s.Save(k);

                    Saved = true;
                    return true;
                }
                return false;
            }
        }

        [DataMember]
        public bool AutoUpdate
        {
            get => _autoUpdate.Get();
            set
            {
                if (_autoUpdate.Set(value)) Saved = false;
            }
        }
        private readonly IProperty<bool> _autoUpdate = H.Property<bool>();

        [DataMember]
        public MonitorSource PrimarySource => _primarySource.Get();

        private readonly IProperty<MonitorSource> _primarySource = H.Property<MonitorSource>(c => c
            .Set(e => e.AllSources.FirstOrDefault(s => s.Primary))
            .On(e => e.AllSources.Item().Primary)
            .Update()
        );


        [DataMember]
        public Rect InMmOutsideBounds => _inMmOutsideBounds.Get();
        private readonly IProperty<Rect> _inMmOutsideBounds = H.Property<Rect>(c => c
           .Set(e =>
               {
                   var outside = new Rect();

                   var first = true;
                   foreach (var s in e.AllMonitors)
                   {
                       if (first)
                       {
                           if (s.InMm == null) continue;
                           outside = s.InMm.OutsideBounds;
                           first = false;
                           continue;
                       }

                       outside.Union(s.InMm.OutsideBounds);
                   }

                   return outside;
               }
           )
           .On(e => e.AllMonitors.Item().InMm.OutsideBounds)
           .Update()
        );



        /// <summary>
        /// Mm Bounds of overall screens without borders
        /// </summary>
        [DataMember]
        public Rect PhysicalBounds => _physicalBounds.Get();
        private readonly IProperty<Rect> _physicalBounds = H.Property<Rect>(c => c
           .Set(e =>
               {
                   var inside = new Rect();

                   var first = true;
                   foreach (var s in e.AllMonitors)
                   {
                       if (first)
                       {
                           if (s.InMm == null) continue;
                           inside = s.InMm.Bounds;
                           first = false;
                           continue;
                       }

                       inside.Union(s.InMm.Bounds);
                   }

                   return inside;
               }
           )
           .On(e => e.AllMonitors.Item().InMm.Bounds)
           .Update()
        );

        public double X0 => _x0.Get();
        private readonly IProperty<double> _x0 =
            H.Property<double>(c => c.Set(e => -e.InMmOutsideBounds.Left).On(e => e.InMmOutsideBounds).Update());

        public double Y0 => _y0.Get();
        private readonly IProperty<double> _y0 =
            H.Property<double>(c => c.Set(e => -e.InMmOutsideBounds.Top).On(e => e.InMmOutsideBounds).Update());

        /// <summary>
        /// 
        /// </summary>

        [DataMember]
        public bool Enabled
        {
            get => _enabled.Get();
            set { if (_enabled.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _enabled = H.Property<bool>();

        [DataMember]
        public bool LoadAtStartup
        {
            get => _loadAtStartup.Get();
            set { if (_loadAtStartup.Set(value)) Saved = false; }
        }

        private readonly IProperty<bool> _loadAtStartup = H.Property<bool>();

        [DataMember]
        public bool LoopAllowed => true;


        [DataMember]
        public bool LoopX
        {
            get => _loopX.Get() && LoopAllowed;
            set { if (_loopX.Set(value)) { Saved = false; } }
        }
        private readonly IProperty<bool> _loopX = H.Property<bool>(c => c
            .On(e => e.LoopAllowed));


        [DataMember]
        public bool LoopY
        {
            get => LoopAllowed && _loopY.Get();
            set { if (_loopY.Set(value)) { Saved = false; } }
        }
        private readonly IProperty<bool> _loopY = H.Property<bool>(c => c
             .On(e => e.LoopAllowed));


        [DataMember]
        public bool IsRatio100 => _isRatio100.Get();
        private readonly IProperty<bool> _isRatio100 = H.Property<bool>(c => c
            .Set(e => e._getIsRatio100())
            .On(e => e.AllSources.Item().PixelToDipRatio)
            .Update()
        );

        private bool _getIsRatio100()
        {
            foreach (var source in AllSources)
            {
                if (source.PixelToDipRatio.X != 1) return false;
                if (source.PixelToDipRatio.Y != 1) return false;
            }
            return true;
        }


        [DataMember]
        public bool AdjustPointerAllowed => _adjustPointerAllowed.Get();
        private readonly IProperty<bool> _adjustPointerAllowed = H.Property<bool>(c => c
            .Set(e => e.IsRatio100)
            .On(e => e.IsRatio100)
            .Update()
        );


        [DataMember]
        public bool AdjustPointer
        {
            get => AdjustPointerAllowed && _adjustPointer.Get();
            set { if (_adjustPointer.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _adjustPointer = H.Property<bool>();

        [DataMember]
        [TriggerOn(nameof(IsRatio100))]
        public bool AdjustSpeedAllowed => _adjustSpeedAllowed.Get();
        private readonly IProperty<bool> _adjustSpeedAllowed = H.Property<bool>(c => c
            .Set(e => e.IsRatio100)
            .On(e => e.IsRatio100)
            .Update()
        );

        [DataMember]
        public bool AdjustSpeed
        {
            get => AdjustSpeedAllowed && _adjustSpeed.Get();
            set { if (_adjustSpeed.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _adjustSpeed = H.Property<bool>();

        [DataMember]
        public bool AllowCornerCrossing
        {
            get => _allowCornerCrossing.Get();
            set { if (_allowCornerCrossing.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _allowCornerCrossing = H.Property<bool>();

        [DataMember]
        public bool HomeCinema
        {
            get => _homeCinema.Get();
            set { if (_homeCinema.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _homeCinema = H.Property<bool>();

        [DataMember]
        public bool Pinned
        {
            get => _pinned.Get();
            set { if (_pinned.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _pinned = H.Property<bool>();

        [DataMember]
        public Rect ConfigLocation
        {
            get => _configLocation.Get();
            set => _configLocation.Set(value);
        }
        private readonly IProperty<Rect> _configLocation = H.Property<Rect>();


        public void SetPhysicalAuto(bool placeall = true)
        {
            if (PrimarySource == null) return;

            lock (_compactLock)
            {
                // List all screens not positioned
                List<Monitor> unatachedScreens = placeall ? AllMonitors.ToList() : AllMonitors.Where(s => !s.Placed).ToList();

                // start with primary screen
                Queue<Monitor> todo = new Queue<Monitor>();
                todo.Enqueue(PrimarySource.Monitor);

                while (todo.Count > 0)
                {
                    foreach (Monitor s2 in todo)
                    {
                        unatachedScreens.Remove(s2);
                    }

                    Monitor placedScreen = todo.Dequeue();

                    foreach (Monitor screenToPlace in unatachedScreens)
                    {
                        if (screenToPlace == placedScreen) continue;

                        bool somethingDone = false;

                        //     __
                        //  __| A
                        // B  |__
                        //  __|
                        if (screenToPlace.ActiveSource.InPixel.Bounds.X == placedScreen.ActiveSource.InPixel.Bounds.Right)
                        {
                            screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Right + screenToPlace.InMm.LeftBorder;
                            somethingDone = true;
                        }
                        //B |___|_
                        //A  |    |
                        if (screenToPlace.ActiveSource.InPixel.Bounds.Y == placedScreen.ActiveSource.InPixel.Bounds.Bottom)
                        {
                            screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Bottom + screenToPlace.InMm.TopBorder;
                            somethingDone = true;
                        }

                        //     __
                        //  __| B
                        // A  |__
                        //  __|
                        if (screenToPlace.ActiveSource.InPixel.Bounds.Right == placedScreen.ActiveSource.InPixel.Bounds.X)
                        {
                            screenToPlace.InMm.X = placedScreen.InMm.OutsideBounds.Left -
                                                      screenToPlace.InMm.OutsideBounds.Width + screenToPlace.InMm.LeftBorder;
                            somethingDone = true;
                        }

                        //A |___|_
                        //B  |    |

                        if (screenToPlace.ActiveSource.InPixel.Bounds.Bottom == placedScreen.ActiveSource.InPixel.Y)
                        {
                            screenToPlace.InMm.Y = placedScreen.InMm.OutsideBounds.Top -
                                                      screenToPlace.InMm.OutsideBounds.Height + screenToPlace.InMm.TopBorder;
                            somethingDone = true;
                        }


                        //  __
                        // |
                        // |__
                        //  __
                        // |
                        // |__
                        if (screenToPlace.ActiveSource.InPixel.Bounds.X == placedScreen.ActiveSource.InPixel.Bounds.X)
                        {
                            screenToPlace.InMm.X = placedScreen.InMm.X;
                            somethingDone = true;
                        }

                        //  ___   ___
                        // |   | |   |
                        if (screenToPlace.ActiveSource.InPixel.Bounds.Y == placedScreen.ActiveSource.InPixel.Bounds.Y)
                        {
                            screenToPlace.InMm.Y = placedScreen.InMm.Y;
                            somethingDone = true;
                        }

                        // __
                        //   |
                        // __|
                        // __
                        //   |
                        // __|
                        if (screenToPlace.ActiveSource.InPixel.Bounds.Right == placedScreen.ActiveSource.InPixel.Bounds.Right)
                        {
                            screenToPlace.InMm.X = placedScreen.InMm.Bounds.Right - screenToPlace.InMm.Bounds.Width;
                            somethingDone = true;
                        }

                        //|___||___|
                        if (screenToPlace.ActiveSource.InPixel.Bounds.Bottom == placedScreen.ActiveSource.InPixel.Bounds.Bottom)
                        {
                            screenToPlace.InMm.Y = placedScreen.InMm.Bounds.Bottom -
                                                      screenToPlace.InMm.Bounds.Height;
                            somethingDone = true;
                        }
                        if (somethingDone)
                        {
                            todo.Enqueue(screenToPlace);
                        }
                    }
                }
            }
        }

        private readonly object _compactLock = new object();
        private bool _compacting;

        public void Compact()
        {
            //            return;
            if (PrimarySource == null) return;


            //if (Moving) return;
            lock (_compactLock)
            {
                List<Monitor> done = new List<Monitor> { PrimarySource.Monitor };

                List<Monitor> todo = AllBut(PrimarySource.Monitor).OrderBy(s => s.Distance(PrimarySource.Monitor)).ToList();

                while (todo.Count > 0)
                {
                    Monitor screen = todo[0];
                    todo.Remove(screen);

                    screen.PlaceAuto(done);
                    done.Add(screen);

                    todo = todo.OrderBy(s => s.Distance(done)).ToList();
                }
            }
        }

        [DataMember]
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
        private readonly IProperty<bool> _allowOverlaps = H.Property<bool>();

        [DataMember]
        public bool AllowDiscontinuity
        {
            get => _allowDiscontinuity.Get();
            set { if (_allowDiscontinuity.Set(value)) Saved = false; }
        }
        private readonly IProperty<bool> _allowDiscontinuity = H.Property<bool>();


        public bool Saved
        {
            get => _saved.Get();
            set => _saved.Set(value);
        }
        private readonly IProperty<bool> _saved = H.Property<bool>();

        [DataMember]
        public double MaxEffectiveDpiX => _maxEffectiveDpiX.Get();
        private readonly IProperty<double> _maxEffectiveDpiX = H.Property<double>(c => c
            .Set(e =>
            {
                var max = 0.0;
                foreach (var source in e.AllSources)
                {
                    if (source.EffectiveDpi != null && source.EffectiveDpi.X > max) max = source.EffectiveDpi.X;
                }
                return max;
            })
            .On(e => e.AllSources.Item().EffectiveDpi.X)
            .Update()
        );

        [DataMember]
        public double MaxEffectiveDpiY => _maxEffectiveDpiY.Get();
        private readonly IProperty<double> _maxEffectiveDpiY = H.Property<double>(c => c
            .Set(e =>
            {
                var max = 0.0;
                foreach (var source in e.AllSources)
                {
                    if (source.EffectiveDpi != null && source.EffectiveDpi.Y > max) max = source.EffectiveDpi.Y;
                }
                return max;
            })
            .On(e => e.AllSources.Item().EffectiveDpi.Y)
            .Update()
        );


        public ConcurrentDictionary<string, MonitorModel> ScreenModels = new ConcurrentDictionary<string, MonitorModel>();

        public MonitorModel GetScreenModel(string pnpCode, MonitorDevice monitor)
        {
            if (string.IsNullOrWhiteSpace(pnpCode)) return null;
            return ScreenModels.GetOrAdd(pnpCode, s => new MonitorModel(s, this).Load(monitor));
        }

        private string ServiceName { get; } = "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');

        private string DaemonExe { get; } = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName
            .Replace(".vshost", "")
            .Replace(".Control.Loader", ".Daemon")
            + ".exe"
            ;


        public bool IsScheduled()
        {
            using var ts = new TaskService();
            return ts.RootFolder.GetTasks(new Regex(ServiceName)).Any();
        }


        public bool Schedule()
        {
            Unschedule();
            using var ts = new TaskService();
            ts.RootFolder.DeleteTask(ServiceName, false);

            var td = ts.NewTask();
            td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
            td.Triggers.Add(
                //new BootTrigger());
                new LogonTrigger { UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name });


            td.Actions.Add(
                new ExecAction(DaemonExe, "--start", AppDomain.CurrentDomain.BaseDirectory)
            );

            td.Principal.RunLevel = TaskRunLevel.Highest;
            td.Settings.DisallowStartIfOnBatteries = false;
            td.Settings.DisallowStartOnRemoteAppSession = true;
            td.Settings.StopIfGoingOnBatteries = false;
            td.Settings.ExecutionTimeLimit = TimeSpan.Zero;
            try
            {
                ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
                return true;
            }
            catch (UnauthorizedAccessException e)
            {
                MessageBox.Show("Unable to register startup task");
                return false;
            }
        }

        public void Unschedule()
        {
            using TaskService ts = new TaskService();
            ts.RootFolder.DeleteTask(ServiceName, false);
        }

        public ZonesLayout ComputeZones()
        {
            var zones = new ZonesLayout();
            foreach (var source in AllSources)
            {
                if (source == source.Monitor.ActiveSource)
                    zones.Zones.Add( new Zone(
                        source.Device.DeviceId,
                        source.Device.Edid.Model,
                        source.InPixel.Bounds,
                        source.Monitor.InMm.Bounds
                        ));
            }

            Zone?[] actualZones = zones.Zones.ToArray();

            if (LoopX)
            {
                var shiftLeft = new Matrix();
                shiftLeft.Translate(-InMmOutsideBounds.Width, 0);
                var shiftRight = new Matrix();
                shiftRight.Translate(InMmOutsideBounds.Width, 0);

                foreach (var zone in actualZones)
                {
                    zones.Zones.Add(new Zone(zone.DeviceId,zone.Name, zone.PixelsBounds, Rect.Transform(zone.PhysicalBounds, shiftLeft), zone));
                    zones.Zones.Add(new Zone(zone.DeviceId,zone.Name, zone.PixelsBounds, Rect.Transform(zone.PhysicalBounds, shiftRight), zone));
                }
            }

            if (LoopY)
            {
                var shiftUp = new Matrix();
                shiftUp.Translate(0, -InMmOutsideBounds.Height);
                var shiftDown = new Matrix();
                shiftDown.Translate(0, InMmOutsideBounds.Height);

                foreach (var zone in actualZones)
                {
                    zones.Zones.Add(new Zone(zone.DeviceId,zone.Name, zone.PixelsBounds, Rect.Transform(zone.PhysicalBounds, shiftUp), zone));
                    zones.Zones.Add(new Zone(zone.DeviceId,zone.Name, zone.PixelsBounds, Rect.Transform(zone.PhysicalBounds, shiftDown), zone));
                }
            }

            zones.Init();

            zones.AdjustPointer = AdjustPointer;
            zones.AdjustSpeed = AdjustSpeed;

            return zones;
        }
    }
}
