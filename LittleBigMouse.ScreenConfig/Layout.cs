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
using System.Reactive.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.Serialization;
using System.Text.RegularExpressions;
using Avalonia;
using DynamicData;
using DynamicData.Alias;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using JetBrains.Annotations;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.Zoning;

using Microsoft.Win32;
using Microsoft.Win32.TaskScheduler;
using ReactiveUI;
using static HLab.Sys.Windows.API.User32;

// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LittleBigMouse.DisplayLayout
{
    internal class SystemEventProxy
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
        ISourceList<Monitor> AllMonitors { get; }
        ZonesLayout ComputeZones();
        bool Save();
    }

    [DataContract]
    public class Layout : ReactiveObject, IMonitorsLayout
    {
        static readonly SystemEventProxy SystemEventProxy = new SystemEventProxy();

        public Layout(IMonitorsService monitorsService)
        {
            MonitorsService = monitorsService;

            _dpiAwarenessContext = GetThreadDpiAwarenessContext();

            //this.WhenAnyValue(
            //        e => e.AllMonitors.Item().BottomBorder,
            //        e => e.Ratio.Y,

            //        (height,r) => height*r
            //    )
            //    .ToProperty(this, e => e.BottomBorder,out _bottomBorder);

            //this.WhenAnyValue(e => e.AutoUpdate, a => false).ToProperty(this, e => e.Saved, out _saved);

            //this.WhenAnyValue(e => e.AllSources.Item());

            this.WhenAnyValue(e => e.InMmOutsideBounds.Left,left => -left).ToProperty(this, e => e.X0, out _x0);
            this.WhenAnyValue(e => e.InMmOutsideBounds.Top,top => -top).ToProperty(this, e => e.Y0, out _y0);

            this.WhenAnyValue(e => e.IsRatio100, (bool r) => r)
                .ToProperty(this, e => e.AdjustPointerAllowed, out _adjustPointerAllowed);

            this.WhenAnyValue(e => e.IsRatio100, (bool r) => r)
                .ToProperty(this, e => e.AdjustSpeedAllowed, out _adjustSpeedAllowed);

            //.On(e => e.AllMonitors.Item().InMm.OutsideBounds)
            var t = AllMonitors.Connect().Select(e => e.InMm.OutsideBounds).Aggregate(e => Union(e)).ToProperty(_inMmOutsideBounds));


            SetWallPaper();

            //NotifyHelper.EventHandlerService.AddHandler<SystemEventProxy, UserPreferenceChangedEventArgs>(SystemEventProxy, "UserPreferenceChanged", SystemEvents_UserPreferenceChanged);

            MonitorsOnCollectionChanged(monitorsService.AttachedMonitors,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, monitorsService.AttachedMonitors.Items.ToList()));

            //NotifyHelper.EventHandlerService.AddHandler(monitorsService.AttachedMonitors, MonitorsOnCollectionChanged);


        }

        internal IMonitorsService MonitorsService { get; }
        public DPI_Awareness_Context DpiAwarenessContext => _dpiAwarenessContext;
        readonly DPI_Awareness_Context _dpiAwarenessContext;

        [DataMember]
        public string Id => _id.Value;
        readonly ObservableAsPropertyHelper<string> _id;

        static string GetId(IEnumerable<Monitor> monitors)
        {
            return monitors.OrderBy(s => s.Id)
                .Aggregate("", (current, screen) => current + (current != "" ? "." : "") + screen.Id);
        }

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

        public MonitorSource MonitorSourceFromPixel(Point pixel) => AllSources.Items.FirstOrDefault(source => source.InPixel.Bounds.Contains(pixel));

        public Monitor MonitorFromPhysicalPosition(Point mm) => AllMonitors.Items.FirstOrDefault(screen => screen.InMm.Bounds.Contains(mm));


        void SystemEvents_UserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
        {
            SetWallPaper();
        }


        public string WallPaperPath
        {
            get => _wallPaperPath;
            private set => this.RaiseAndSetIfChanged(ref _wallPaperPath, value);
        }
        string _wallPaperPath;

        void SetWallPaper()
        {
            var currentWallpaper = new string('\0', SetupApi.MAX_PATH);
            SystemParametersInfo(SPI_GETDESKWALLPAPER, currentWallpaper.Length, currentWallpaper, 0);

            WallPaperPath = currentWallpaper[..currentWallpaper.IndexOf('\0')];

            using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
            {
                if (key != null)
                {
                    TileWallpaper = key.GetValue("TileWallpaper", "0").ToString() == "1";
                    if (int.TryParse(key.GetValue("WallpaperStyle", "0").ToString(), out var value))
                    {
                        WallpaperStyle = value;
                    }
                }
            }

            using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Colors", false))
            {
                if (key == null)
                    BackgroundColor = new[] { 0, 0, 0 };
                else
                {
                    var s = key.GetValue("Background", "0 0 0").ToString();
                    BackgroundColor = s?.Split(' ').Select(int.Parse).ToArray() ?? new[] { 0, 0, 0 };
                }
            }
        }

        public bool TileWallpaper
        {
            get => _tileWallpaper;
            private set => this.RaiseAndSetIfChanged(ref _tileWallpaper, value);
        }
        bool _tileWallpaper;

        public int WallpaperStyle
        {
            get => _wallpaperStyle;
            private set => this.RaiseAndSetIfChanged(ref _wallpaperStyle, value);
        }
        int _wallpaperStyle;

        public int[] BackgroundColor
        {
            get => _backgroundColor;
            private set => this.RaiseAndSetIfChanged(ref _backgroundColor, value);
        }
        int[] _backgroundColor;

        void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            switch (args.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    if (args.NewItems != null)
                        foreach (var device in args.NewItems.OfType<MonitorDevice>())
                        {
                            var source = AllSources.Items.FirstOrDefault(s => s.Device.Equals(device));
                            if (source != null) continue;

                            var monitor = AllMonitors.Items.FirstOrDefault(m => 
                                m.Model.PnpCode == device.PnpCode 
                                && m.ActiveSource.Device.IdPhysicalMonitor == device.IdPhysicalMonitor
                                );

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
                            var screen = AllSources.Items.FirstOrDefault(s => s.Monitor.Equals(monitor));

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
        public ISourceList<Monitor> AllMonitors { get; } = new SourceList<Monitor>();
        [NotNull] public ISourceList<MonitorSource> AllSources { get; } = new SourceList<MonitorSource>();

        public IEnumerable<Monitor> AllBut(Monitor screen) => AllMonitors.Items.Where(s => !Equals(s, screen));

        public Monitor Selected => _selected.Get();

        readonly ObservableAsPropertyHelper<Monitor> _selected;

        //.On(e => e.AllMonitors.Item().Selected)
        static Monitor GetSelected(IEnumerable<Monitor> monitors)
        {
            return monitors.FirstOrDefault(screen => screen.Selected);
        }

        const string ROOT_KEY = @"SOFTWARE\Mgth\LittleBigMouse";

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

                    foreach (var source in AllSources.Items)
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
                        var m = MonitorsService.Monitors.Items.FirstOrDefault(
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
                if (monkey is not null)
                {
                    area = new(
                        double.Parse(monkey.GetValue("PixelX").ToString()),
                        double.Parse(monkey.GetValue("PixelY").ToString()),
                        double.Parse(monkey.GetValue("PixelWidth").ToString()),
                        double.Parse(monkey.GetValue("PixelHeight").ToString()));

                    primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
                    orientation = (int)double.Parse(monkey.GetValue("Orientation").ToString());
                }
            }

            var monitor = MonitorsService.Monitors.Items.FirstOrDefault(
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
                    foreach (Monitor s in AllMonitors.Items)
                    {
                        s.Load(key);
                    }
                }
            }
            Saved = true;
        }

        public bool Save()
        {
            using RegistryKey k = OpenRegKey(true);
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

                foreach (var monitor in AllMonitors.Items)
                    monitor.Save(k);

                Saved = true;
                return true;
            }
            return false;
        }

        [DataMember]
        public bool AutoUpdate
        {
            get => _autoUpdate;
            set => SetValue(ref _autoUpdate, value);
        }
        bool _autoUpdate;

        [DataMember]
        public MonitorSource PrimarySource => _primarySource.Value;

        readonly ObservableAsPropertyHelper<MonitorSource> _primarySource; 

        //.On(e => e.AllSources.Item().Primary)
        static MonitorSource GetPrimarySource(IEnumerable<MonitorSource> sources) => sources.FirstOrDefault(s => s.Primary);

        [DataMember]
        public Rect InMmOutsideBounds => _inMmOutsideBounds.Value;
        readonly ObservableAsPropertyHelper<Rect> _inMmOutsideBounds;

        //.On(e => e.AllMonitors.Item().InMm.OutsideBounds)
        static Rect GetInMmOutsideBounds(IEnumerable<Monitor> monitors)
        {
            return Union(monitors.Select(e => e.InMm.OutsideBounds));
        }

        public static Rect Union(IEnumerable<Rect> rects)
        {
            var outside = new Rect();

            var first = true;
            foreach (var rect in rects)
            {
                if (first)
                {
                    outside = rect;
                    first = false;
                    continue;
                }

                outside.Union(rect);
            }

            return outside;
        }



        /// <summary>
        /// Mm Bounds of overall screens without borders
        /// </summary>
        [DataMember]
        public Rect PhysicalBounds => _physicalBounds.Get();
        readonly ObservableAsPropertyHelper<Rect> _physicalBounds;

        // .On(e => e.AllMonitors.Item().InMm.Bounds)
        static Rect GetPhysicalBounds(IEnumerable<Monitor> monitors)
        {
            var inside = new Rect();

            var first = true;
            foreach (var s in monitors)
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

        public double X0 => _x0.Get();

        readonly ObservableAsPropertyHelper<double> _x0;

        public double Y0 => _y0.Get();
        readonly ObservableAsPropertyHelper<double> _y0;

        /// <summary>
        /// 
        /// </summary>
        bool SetValue<TRet>(ref TRet backingField, TRet value, [CallerMemberName] string propertyName = null)
        {
            using (DelayChangeNotifications())
            {
                if (EqualityComparer<TRet>.Default.Equals(backingField, value))
                {
                    this.RaisePropertyChanging(propertyName);
                    backingField = value;
                    Saved = false;
                    this.RaisePropertyChanged(propertyName);
                    return true;
                }

                return false;
            }
        }


        [DataMember]
        public bool Enabled
        {
            get => _enabled;
            set => SetValue(ref _enabled, value);
        }

        bool _enabled;

        [DataMember]
        public bool LoadAtStartup
        {
            get => _loadAtStartup;
            set => SetValue(ref _loadAtStartup, value);
        }
        bool _loadAtStartup;

        [DataMember]
        public bool LoopAllowed => true;


        [DataMember]
        public bool LoopX
        {
            get => LoopAllowed && _loopX;
            set => SetValue(ref _loopX, value);
        }
        bool _loopX;


        [DataMember]
        public bool LoopY
        {
            get => LoopAllowed && _loopY;
            set => SetValue(ref _loopY, value);
        }
        bool _loopY;


        [DataMember]
        public bool IsRatio100 => _isRatio100.Get();
        readonly ObservableAsPropertyHelper<bool> _isRatio100;

        //.On(e => e.AllSources.Item().PixelToDipRatio)
        static bool _getIsRatio100(IEnumerable<MonitorSource> sources)
        {
            foreach (var source in sources)
            {
                if (source.PixelToDipRatio.X != 1) return false;
                if (source.PixelToDipRatio.Y != 1) return false;
            }
            return true;
        }


        [DataMember]
        public bool AdjustPointerAllowed => _adjustPointerAllowed.Get();
        readonly ObservableAsPropertyHelper<bool> _adjustPointerAllowed;


        [DataMember]
        public bool AdjustPointer
        {
            get => AdjustPointerAllowed && _adjustPointer;
            set => SetValue(ref _adjustPointer, value);
        }

        bool _adjustPointer;

        [DataMember]
        public bool AdjustSpeedAllowed => _adjustSpeedAllowed.Get();
        readonly ObservableAsPropertyHelper<bool> _adjustSpeedAllowed;

        [DataMember]
        public bool AdjustSpeed
        {
            get => AdjustSpeedAllowed && _adjustSpeed;
            set => SetValue(ref _adjustSpeed, value);
        }
        bool _adjustSpeed;

        [DataMember]
        public bool AllowCornerCrossing
        {
            get => _allowCornerCrossing;
            set => SetValue(ref _allowCornerCrossing, value);
        }

        bool _allowCornerCrossing;

        [DataMember]
        public bool HomeCinema
        {
            get => _homeCinema;
            set => SetValue(ref _homeCinema, value);
        }
        bool _homeCinema;

        [DataMember]
        public bool Pinned
        {
            get => _pinned;
            set => SetValue(ref _pinned, value);
        }
        bool _pinned;

        [DataMember]
        public Rect ConfigLocation
        {
            get => _configLocation;
            set => SetValue(ref _configLocation, value);
        }
        Rect _configLocation;


        public void SetPhysicalAuto(bool placeall = true)
        {
            if (PrimarySource == null) return;

            lock (_compactLock)
            {
                // List all screens not positioned
                List<Monitor> unatachedScreens = placeall ? AllMonitors.Items.ToList() : AllMonitors.Items.Where(s => !s.Placed).ToList();

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

        readonly object _compactLock = new object();
        bool _compacting;

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
            get => _allowOverlaps;
            set => SetValue(ref _allowOverlaps, value);
        }
        bool _allowOverlaps;

        [DataMember]
        public bool AllowDiscontinuity
        {
            get => _allowDiscontinuity;
            set => SetValue(ref _allowDiscontinuity, value);
        }
        bool _allowDiscontinuity;


        public bool Saved
        {
            get => _saved;
            set => this.RaiseAndSetIfChanged(ref _saved, value);
        }
        bool _saved;

        [DataMember]
        public double MaxEffectiveDpiX => _maxEffectiveDpiX.Get();

        readonly ObservableAsPropertyHelper<double> _maxEffectiveDpiX;

        //.On(e => e.AllSources.Item().EffectiveDpi.X)
        static double GetMaxEffectiveDpiX(IEnumerable<MonitorSource> sources) => sources
            .Where(s => s.EffectiveDpi is not null)
            .Select(s => s.EffectiveDpi.X)
            .Max();

        [DataMember]
        public double MaxEffectiveDpiY => _maxEffectiveDpiY.Get();
        readonly ObservableAsPropertyHelper<double> _maxEffectiveDpiY;
        static double GetMaxEffectiveDpiY(IEnumerable<MonitorSource> sources) => sources
            .Where(s => s.EffectiveDpi is not null)
            .Select(s => s.EffectiveDpi.Y)
            .Max();

        //.On(e => e.AllSources.Item().EffectiveDpi.Y)


        public ConcurrentDictionary<string, MonitorModel> ScreenModels = new ConcurrentDictionary<string, MonitorModel>();

        public MonitorModel GetScreenModel(string pnpCode, MonitorDevice monitor)
        {
            if (string.IsNullOrWhiteSpace(pnpCode)) return null;
            return ScreenModels.GetOrAdd(pnpCode, s => new MonitorModel(s, this).Load(monitor));
        }

        string ServiceName { get; } = "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');

        string DaemonExe { get; } = AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName
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
                // Todo avalonia
                //MessageBox.Show("Unable to register startup task");
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
            foreach (var source in AllSources.Items)
            {
                if (source == source.Monitor.ActiveSource)
                    zones.Zones.Add(new Zone(
                        source.Device.DeviceId,
                        source.Device.Edid.Model,
                        source.InPixel.Bounds,
                        source.Monitor.InMm.Bounds
                        ));
            }

            Zone?[] actualZones = zones.Zones.ToArray();

            if (LoopX)
            {
                var shiftLeft = new Vector(-InMmOutsideBounds.Width, 0);
                var shiftRight = new Vector(InMmOutsideBounds.Width, 0);

                foreach (var zone in actualZones)
                {
                    zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftLeft), zone));
                    zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftRight), zone));
                }
            }

            if (LoopY)
            {
                var shiftUp = new Vector(0, -InMmOutsideBounds.Height);
                var shiftDown = new Vector(0, InMmOutsideBounds.Height);

                foreach (var zone in actualZones)
                {
                    zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftUp), zone));
                    zones.Zones.Add(new Zone(zone.DeviceId, zone.Name, zone.PixelsBounds, zone.PhysicalBounds.Translate(shiftDown), zone));
                }
            }

            zones.Init();

            zones.AdjustPointer = AdjustPointer;
            zones.AdjustSpeed = AdjustSpeed;

            return zones;
        }
    }
}
