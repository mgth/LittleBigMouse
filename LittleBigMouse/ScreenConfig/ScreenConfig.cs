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
using HLab.Notify;
using HLab.Windows.API;
using HLab.Windows.Monitors;
using Microsoft.Win32;
using Newtonsoft.Json;

// ReSharper disable CompareOfFloatsByEqualityOperator

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenConfig : NotifierObject
    {
        public static RegistryKey OpenRootRegKey(bool create = false)
        {
            using (RegistryKey key = Registry.CurrentUser)
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(RootKey) : key.OpenSubKey(RootKey);
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



        public ScreenConfig(IMonitorsService monitorsService) : base(false)
        {
            this.SubscribeNotifier();

            SystemEvents.UserPreferenceChanged += SystemEvents_UserPreferenceChanged;

            MonitorsOnCollectionChanged(monitorsService.AttachedMonitors,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, MonitorsService.D.AttachedMonitors));

            monitorsService.AttachedMonitors.CollectionChanged += MonitorsOnCollectionChanged;
        }

        private void SystemEvents_UserPreferenceChanged(object sender, UserPreferenceChangedEventArgs e)
        {
            Notifier.Update(GetType().GetProperty("WallPaperPath"));
        }

        public string WallPaperPath => this.Get(GetCurrentDesktopWallpaper);

        public string GetCurrentDesktopWallpaper()
        {
            string currentWallpaper = new string('\0', NativeMethods.MAX_PATH);
            NativeMethods.SystemParametersInfo(NativeMethods.SPI_GETDESKWALLPAPER, currentWallpaper.Length, currentWallpaper, 0);
            return currentWallpaper.Substring(0, currentWallpaper.IndexOf('\0'));
        }

        public bool TiledWallPaper => this.Get(() =>
        {
            using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
            {
                if (key == null) return false;
                return key.GetValue("WallpaperStyle","0").ToString() == "1";
            }
        });

        public int WallpaperStyle => this.Get(() =>
        {
            using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop", false))
            {
                if (key == null) return 0;

                if(int.TryParse(key.GetValue("WallpaperStyle","0").ToString(),out var value))
                {
                    return value;
                }
                return 0;
            }
        });

        public int[] BackGroundColor => this.Get<int[]>(() =>
        {
            using (var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Colors", false))
            {
                if (key == null) return new []{0,0,0};
                var s = key.GetValue("Background", "0 0 0").ToString();
                var ss = s.Split(' ');
                var i = ss.Select(int.Parse).ToArray();
                return i;
            }
        });

        private void MonitorsOnCollectionChanged(object sender, NotifyCollectionChangedEventArgs args)
        {
            if (args.NewItems != null)
                foreach (var monitor in args.NewItems.OfType<Monitor>())
                {
                    var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));
                    if (screen != null) continue;

                    screen = new Screen(this, monitor);
                    AllScreens.Add(screen);
                }
            if (args.OldItems != null)
                foreach (var monitor in args.OldItems.OfType<Monitor>())
                {
                    var screen = AllScreens.FirstOrDefault(s => s.Monitor.Equals(monitor));

                    if (screen != null) AllScreens.Remove(screen);
                }

            Load();
            SetPhysicalAuto(false);
        }

        [JsonProperty]
        public ObservableCollection<Screen> AllScreens => this.Get(()=>new ObservableCollection<Screen>());

        public IEnumerable<Screen> AllBut(Screen screen) => AllScreens.Where(s => !Equals(s, screen));

        [TriggedOn(nameof(AllScreens),"Item","Selected")]
        public Screen Selected => this.Get(()=>AllScreens.FirstOrDefault(screen => screen.Selected));


        private const string RootKey = @"SOFTWARE\Mgth\LittleBigMouse";

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

        [JsonProperty]
        [TriggedOn(nameof(AllScreens),"Item","Id")]
        public string Id => this.Get(() => AllScreens.OrderBy(s => s.Id)
                    .Aggregate("", (current, screen) => current + (current != "" ? "." : "") + screen.Id));

        public void MatchConfig(string id)
        {
            using (var rootkey = OpenRootRegKey())
            {
                using (var key = rootkey.OpenSubKey(@"configs\" + id))
                {
                    var todo = key.GetSubKeyNames().ToList();

                    foreach (var screen in AllScreens)
                    {
                        if (todo.Contains(screen.IdMonitor))
                        {
                            AttachToDesktop(id, screen.IdMonitor, false);
                            todo.Remove(screen.IdMonitor);
                        }
                        else
                        {
                            MonitorsService.D.DetachFromDesktop(screen.Monitor.AttachedDisplay.DeviceName, false);
                        }
                    }

                    foreach (string s in todo)
                    {
                        AttachToDesktop(id, s, false);
                    }

                    MonitorsService.D.ApplyDesktop();
                }
            }
        }

        public static bool IsDoableConfig(String id)
        {
            using (RegistryKey rootkey = OpenRootRegKey())
            {
                using (RegistryKey key = rootkey.OpenSubKey(@"configs\" + id))
                {
                    List<string> todo = key.GetSubKeyNames().ToList();

                    // ReSharper disable once LoopCanBeConvertedToQuery
                    foreach (string s in todo)
                    {
                        //string s2 = s.Substring(0, s.Length - 2);
                        Monitor m = MonitorsService.D.Monitors.FirstOrDefault(
                            d => s == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

                        if (m == null) return false;
                    }
                }
            }
            return true;
        }

        public static void AttachToDesktop(string configId, string monitorId, bool apply = true)
        {
            //using (RegistryKey monkey = Screen.OpenMonitorRegKey(monitorId))
            //{
            //    id = monkey?.GetValue("DeviceId").ToString();
            //    if (id == null) return;
            //}
            Rect area = new Rect();
            bool primary = false;
            int orientation = 0;

            using (RegistryKey monkey = Screen.OpenConfigRegKey(configId, monitorId))
            {
                area.X = double.Parse(monkey.GetValue("PixelX").ToString());
                area.Y = double.Parse(monkey.GetValue("PixelY").ToString());
                area.Width = double.Parse(monkey.GetValue("PixelWidth").ToString());
                area.Height = double.Parse(monkey.GetValue("PixelHeight").ToString());

                primary = double.Parse(monkey.GetValue("Primary").ToString()) == 1;
                orientation = (int) double.Parse(monkey.GetValue("Orientation").ToString());
            }

            Monitor monitor = MonitorsService.D.Monitors.FirstOrDefault(
                d => monitorId == d.Edid.ManufacturerCode + d.Edid.ProductCode + "_" + d.Edid.Serial);

            if(monitor!=null)
            MonitorsService.D.AttachToDesktop(monitor.AttachedDisplay.DeviceName, primary, area, orientation, apply);
        }

        public void EnumWmi()
        {
            string NamespacePath = "\\\\.\\ROOT\\WMI\\ms_409";
            string ClassName = "WmiMonitorID";

            //Create ManagementClass
            ManagementClass oClass = new ManagementClass(NamespacePath + ":" + ClassName);

            //Get all instances of the class and enumerate them
            foreach (var o in oClass.GetInstances().OfType<ManagementObject>())
            {
                //access a property of the Management object
                Console.WriteLine("ManufacturerName : {0}", o["ManufacturerName"]);
            }
        }



        public void Load()
        {
//            Moving = true; //TODO : Hugly hack
            //SetPhysicalAuto();

            using (this.Suspend())
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
                
            }

//            Moving = false;
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

        [JsonProperty]
        public bool AutoUpdate
        {
            get => this.Get<bool>();
            set
            {
                if (this.Set(value)) Saved = false;
            }
        }

        [JsonProperty]
        public Screen PrimaryScreen => AllScreens.FirstOrDefault(s => s.Primary);

        ///// <summary>
        ///// Moving is true when screen is dragged on gui
        ///// </summary>
        //public bool Moving
        //{
        //    get => this.Get<bool>();
        //    set => this.Set(value);
        //}

        /// <summary>
        /// Mm Outside Bounds updated while moving (screen dragged on gui)
        /// </summary>
        //[TriggedOn(nameof(Moving))]
        [JsonProperty]
        [TriggedOn(nameof(AllScreens), "Item", "InMm.OutsideBounds")]
        public Rect PhysicalOutsideBounds => this.Get(() =>
        {
            var outside = new Rect();

            var first = true;
            foreach (var s in AllScreens)
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

        });


        /// <summary>
        /// Mm Outside Bounds NOT updated while moving (screen dragged on gui)
        /// </summary>
        //public Rect MovingPhysicalOutsideBounds
        //{
        //    get => this.Get<Rect>();
        //    private set => this.Set(value);
        //}

        //public void ShiftMovingPhysicalBounds(Vector shift)
        //{
        //    Rect r = new Rect(
        //        MovingPhysicalOutsideBounds.TopLeft + shift
        //        , MovingPhysicalOutsideBounds.Size
        //        );
        //    MovingPhysicalOutsideBounds = r;
        //}

        //[TriggedOn(nameof(Moving))]
        //[TriggedOn(nameof(PhysicalOutsideBounds))]
        //private void UpdateMovingPhysicalOutsideBounds()
        //{
        //    if (Moving) return;
        //    MovingPhysicalOutsideBounds = PhysicalOutsideBounds;
        //}


        /// <summary>
        /// Mm Bounds of overall screens without borders
        /// </summary>
        [JsonProperty]
        [TriggedOn(nameof(AllScreens), "Item", "InMm", "Bounds")]
        public Rect PhysicalBounds => this.Get(() =>
        {
            var inside = new Rect();

            var first = true;
            foreach (var s in AllScreens)
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
        });


        /// <summary>
        /// 
        /// </summary>

        [JsonProperty]
        public bool Enabled
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
        public bool LoadAtStartup
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) { Saved = false; } }
        }

        //[TriggedOn(nameof(AllowCornerCrossing))]
        [JsonProperty]
        public bool LoopAllowed => true;

        [JsonProperty]
        [TriggedOn(nameof(LoopAllowed))]
        public bool LoopX
        {
            get => LoopAllowed && this.Get<bool>();
            set { if (this.Set(value)) { Saved = false; } }
        }

        [JsonProperty]
        [TriggedOn(nameof(LoopAllowed))]
        public bool LoopY
        {
            get => LoopAllowed && this.Get<bool>();
            set { if (this.Set(value)) { Saved = false; } }
        }

        [JsonProperty]
        [TriggedOn(nameof(AllScreens),"Item","PixelToDipRatio")]
        public bool IsRatio100
        {
            get
            {
                foreach (Screen screen in AllScreens)
                {
                    if (screen.PixelToDipRatio.X != 1) return false;
                    if (screen.PixelToDipRatio.Y != 1) return false;
                }
                return true;
            }
        }

        [JsonProperty]
        [TriggedOn(nameof(IsRatio100))]
        public bool AdjustPointerAllowed => IsRatio100;

        [JsonProperty]
        public bool AdjustPointer
        {
            get => AdjustPointerAllowed && this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
        public bool AdjustSpeedAllowed => IsRatio100;


        [JsonProperty]
        public bool AdjustSpeed
        {
            get => AdjustSpeedAllowed && this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
        public bool AllowCornerCrossing
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
        public bool HomeCinema
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
        public bool Pinned
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        [JsonProperty]
       public Rect ConfigLocation
        {
            get => this.Get<Rect>();
            set => this.Set(value);
        }


        public void SetPhysicalAuto(bool placeall = true)
        {
            if (PrimaryScreen == null) return;

            lock (_compactLock)
            {
                if (_compacting) return;
                _compacting = true;
            }
            // List all screens not positioned
            List<Screen> unatachedScreens = placeall?AllScreens.ToList():AllScreens.Where(s => !s.Placed).ToList();

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

            List<Screen> done = new List<Screen> {PrimaryScreen};

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
            get => this.Get<bool>(); set
            {
                if (this.Set(value))
                {
                    Saved = false;
                }
            }
        }

        [JsonProperty]
        public bool AllowDiscontinuity
        {
            get => this.Get<bool>();
            set { if (this.Set(value)) Saved = false; }
        }

        public bool Saved
        {
            get => this.Get<bool>();
            set => this.Set(value);
        }

        [JsonProperty]
        [TriggedOn(nameof(AllScreens),"Item.EffectiveDpi","X")]
        public double MaxEffectiveDpiX => this.Get( 
            ()=>AllScreens.Count==0?0:AllScreens.Select(screen => screen.EffectiveDpi.X).Max());

        [JsonProperty]
        [TriggedOn(nameof(AllScreens),"Item.EffectiveDpi","Y")]
        public double MaxEffectiveDpiY => this.Get( 
            ()=>AllScreens.Count==0?0:AllScreens.Select(screen => screen.EffectiveDpi.Y).Max());


        public ConcurrentDictionary<string,ScreenModel> ScreenModels = new ConcurrentDictionary<string,ScreenModel>();

        public ScreenModel GetScreenModel(string pnpCode, Monitor monitor)
        {
            return ScreenModels.GetOrAdd(pnpCode, s => new ScreenModel(s, this).Load(monitor));
        }
    }
}
