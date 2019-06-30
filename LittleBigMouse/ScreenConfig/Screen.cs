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
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.ServiceModel.PeerResolvers;
using System.Text;
using System.Windows;
using HLab.Base;
using HLab.DependencyInjection.Annotations;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Windows.API;
using HLab.Windows.Monitors;
using Microsoft.Win32;
using Newtonsoft.Json;
using Newtonsoft.Json.Serialization;

[assembly: InternalsVisibleTo("ScreenConfig")]

namespace LittleBigMouse.ScreenConfigs
{
    public class Screen : N<Screen>
    {
        [JsonIgnore] public ScreenConfig Config { get; }
        public Monitor Monitor { get; }

        internal Screen(ScreenConfig config, Monitor monitor)
        {
            Config = config;
            Monitor = monitor;

            Initialize();
        }

        [TriggerOn(nameof(Monitor), "AttachedDisplay")]
        private void UpdateModel()
        {
            ScreenModel.Load(Monitor);
        }


        private readonly IProperty<IEnumerable<Screen>> _otherScreens = H.Property<IEnumerable<Screen>>(c => c
                .On(e => e.Config.AllScreens.Count)
                .Set(e => e.Config.AllScreens.Where(s => !Equals(s, e)))
        );

        [JsonIgnore] public IEnumerable<Screen> OtherScreens => _otherScreens.Get();



        public override bool Equals(object obj)
        {
            return obj is Screen other && Monitor.Equals(other.Monitor);
        }

        public override int GetHashCode()
        {
            return ("Screen" + Monitor.DeviceId).GetHashCode();
        }


        // References properties
        [JsonProperty] public string Id => _id.Get();

        private readonly IProperty<string> _id = H.Property<string>(c => c
            .On(e => e.Monitor.IdMonitor)
            .On(e => e.Orientation)
            .Set(e => e.Monitor.IdMonitor + "_" + e.Orientation)
        );


        [JsonProperty] public string IdResolution => _idResolution.Get();

        private readonly IProperty<string> _idResolution = H.Property<string>(c => c
            .On(e => e.InPixel)
            .Set(e => e.InPixel.Width + "x" + e.InPixel.Height)
        );

        [JsonProperty] public bool Primary => _primary.Get();
        private readonly IProperty<bool> _primary = H.Property<bool>(c => c
            .On(e => e.Monitor.Primary)
            .Set(e => e.Monitor.Primary)
        );

        [JsonProperty] public int Orientation => _orientation.Get();
        private readonly IProperty<int> _orientation = H.Property<int>(c => c
            .On(e => e.Monitor.AttachedDisplay.CurrentMode.DisplayOrientation)
            .Set(s => s.Monitor.AttachedDisplay?.CurrentMode.DisplayOrientation ?? 0)
        );

        public bool Selected
        {
            get => _selected.Get();
            set
            {
                if (!_selected.Set(value)) return;
                if (!value) return;
                foreach (var screen in Config.AllBut(this)) screen.Selected = false;
            }
        }
        private readonly IProperty<bool> _selected = H.Property<bool>(nameof(Selected));

        public bool Placed
        {
            get => _placed.Get();
            set => _placed.Set(value);
        }
        private readonly IProperty<bool> _placed = H.Property<bool>(nameof(Placed));

        // Mm dimensions
        // Natives

        public ScreenModel ScreenModel => _screenModel.Get();

        private readonly IProperty<ScreenModel> _screenModel = H.Property<ScreenModel>(nameof(ScreenModel),
            c => c
                .On(e => e.Monitor.PnpCode)
                .On(e => e.Monitor)
                .NotNull(e => e.Config).NotNull(e => e.Monitor.PnpCode).NotNull(e => e.Monitor)
                .Set(s => s.Config.GetScreenModel(s.Monitor.PnpCode, s.Monitor)));

        [JsonProperty] public IScreenSize PhysicalRotated => _physicalRotated.Get();

        private readonly IProperty<IScreenSize> _physicalRotated =
            H.Property<IScreenSize>(nameof(PhysicalRotated), c => c
                .On(e => e.ScreenModel.Physical)
                .On(e => e.Orientation)
                .NotNull(e => e.ScreenModel)
                .Set(s => s.ScreenModel.Physical.Rotate(s.Orientation)));



        //Mm
        [JsonProperty] public IScreenSize InMm => _inMm.Get();
        private readonly IProperty<IScreenSize> _inMm = H.Property<IScreenSize>(c => c
            .On(e => e.PhysicalRotated)
            .On(e => e.PhysicalRatio)
            .NotNull(e => e.PhysicalRotated)
            .Set(e => e.PhysicalRotated.Scale(e.PhysicalRatio).Locate())
        );


        [TriggerOn(nameof(InMm), "X")]
        public double InMmX
        {
            get => InMm.X;
            set
            {
                if (Primary)
                {
                    foreach (var screen in Config.AllBut(this))
                    {
                        screen.InMm.X -= value;
                    }
                }
                else InMm.X = value;
            }
        }

        public double InMmY
        {
            get => InMm.Y;
            set
            {
                if (Primary)
                {
                    foreach (var screen in Config.AllBut(this))
                    {
                        screen.InMm.Y -= value;
                    }
                }
                else InMm.Y = value;
            }
        }

        [TriggerOn(nameof(InMm), "X")]
        [TriggerOn(nameof(InMm), "Y")]
        public void SetSaved()
        {
            Config.Saved = false;
        }


        [JsonProperty]
        public IScreenSize InMmUnrotated => _inMmUnrotated.Get();
        private readonly IProperty<IScreenSize> _inMmUnrotated = H.Property<IScreenSize>(c => c
            .On(e => e.InMm)
            .On(e => e.Orientation)
            .Set(e => e.InMm.Rotate(4 - e.Orientation))
        );

        //Pixel
        [JsonProperty] public IScreenSize InPixel => _inPixel.Get();
        private readonly IProperty<IScreenSize> _inPixel = H.Property<IScreenSize>(c => c
            .Set(s => new ScreenSizeInPixels(s))
        );


        // Dip
        [JsonProperty] public IScreenSize InDip => _inDip.Get();
        private readonly IProperty<IScreenSize> _inDip = H.Property<IScreenSize>(nameof(InDip), c => c
            .On(e => e.InPixel)
            .Set(s => s.InPixel.ScaleDip(s)));




        [JsonProperty]
        public IScreenRatio RealPitch
        {
            get => _realPitch.Get();
            set
            {
                PhysicalRotated.Width = InPixel.Width * value.X;
                PhysicalRotated.Height = InPixel.Height * value.Y;
            }
        }

        private readonly IProperty<IScreenRatio> _realPitch = H.Property<IScreenRatio>(c => c
            .On(e => e.PhysicalRotated.Width)
            .On(e => e.PhysicalRotated.Height)
            .On(e => e.InPixel.Width)
            .On(e => e.InPixel.Height)
            .Set(e => new ScreenRatioValue(
                e.PhysicalRotated.Width / e.InPixel.Width,
                e.PhysicalRotated.Height / e.InPixel.Height)));

        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [JsonProperty]
        public IScreenRatio PhysicalRatio => _physicalRatio.Get();
        private readonly IProperty<IScreenRatio> _physicalRatio = H.Property<IScreenRatio>(c=>c
                .Set(e => new ScreenRatioValue(1.0, 1.0))
        );

        public double Diagonal => _diagonal.Get();
        private readonly IProperty<double> _diagonal = H.Property<double>(c => c
                .On(e => e.InMm.Height)
                .On(e => e.InMm.Width)
                .Set(e => Math.Sqrt(e.InMm.Width * e.InMm.Width + e.InMm.Height * e.InMm.Height))
        );


        //calculated
        [JsonProperty]
        public IScreenRatio Pitch => _pitch.Get();
        private readonly IProperty<IScreenRatio> _pitch
            = H.Property<IScreenRatio>(c => c
                .On(e => e.RealPitch)
                .On(e => e.PhysicalRatio)
                .Set(e => e.RealPitch.Multiply(e.PhysicalRatio))
            );


        [JsonProperty] public Rect OverallBoundsWithoutThisInMm => _overallBoundsWithoutThisInMm.Get();
        private readonly IProperty<Rect> _overallBoundsWithoutThisInMm 
            = H.Property<Rect>(c => c
                .On(e => e.Config.AllScreens.Item().InMm.Bounds)
                .Set(e =>
                {
                    var result = new Rect();
                    var first = true;
                    foreach (var screen in e.Config.AllBut(e))
                    {
                        if (first)
                        {
                            result = screen.InMm.Bounds;
                            first = false;
                        }
                        else
                            result.Union(screen.InMm.Bounds);
                    }

                    return result;
                })

        ); 







        // Registry settings

        public static RegistryKey OpenMonitorRegKey(string id, bool create = false)
        {
            using (RegistryKey key = ScreenConfig.OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"monitors\" + id) : key.OpenSubKey(@"monitors\" + id);
            }
        }

        public RegistryKey OpenMonitorRegKey(bool create = false)
        {
            return OpenMonitorRegKey(Monitor.IdMonitor, create);
        }

        public RegistryKey OpenGuiLocationRegKey(bool create = false)
        {
            using (RegistryKey key = OpenMonitorRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey("GuiLocation") : key.OpenSubKey("GuiLocation");
            }
        }

        public static RegistryKey OpenConfigRegKey(string configId, string monitorId, bool create = false)
        {
            using (RegistryKey key = ScreenConfig.OpenConfigRegKey(configId, create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(monitorId) : key.OpenSubKey(monitorId);
            }
        }

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Config.Id, Monitor.IdMonitor, create);

        public string ConfigPath(bool create = false)
        {
            string path = Path.Combine(Config.ConfigPath(create), Id);
            if (create) Directory.CreateDirectory(path);

            return path;
        }




        public void Load()
        {
            using (RegistryKey key = OpenGuiLocationRegKey())
            {
                if (key != null)
                {
                    var left = key.GetKey("Left", ()=>GuiLocation.Left);
                    var width = key.GetKey("Width", ()=>GuiLocation.Width);
                    var top = key.GetKey("Top", ()=>GuiLocation.Top);
                    var height = key.GetKey("Height", ()=>GuiLocation.Height);
                    _guiLocation.Set(new Rect(new Point(left, top), new Size(width, height)));
                }
            }

            using (RegistryKey key = OpenConfigRegKey(false))
            {
                if (key != null)
                {
                    InMm.X = key.GetKey("XLocationInMm", () => InMm.X, ()=>Placed=true);
                    InMm.Y = key.GetKey("YLocationInMm", () => InMm.Y, ()=>Placed=true);
                    PhysicalRatio.X = key.GetKey("PhysicalRatioX", () => PhysicalRatio.X);
                    PhysicalRatio.Y = key.GetKey("PhysicalRatioY", () => PhysicalRatio.Y);
                }
            }
        }

        public void Save(RegistryKey baseKey)
        {
            ScreenModel.Save(baseKey);

            using (RegistryKey key = OpenGuiLocationRegKey(true))
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }

            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                key?.SetKey("DeviceId", Monitor.DeviceId);
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey("XLocationInMm", InMm.X);
                    key.SetKey("YLocationInMm", InMm.Y);
                    key.SetKey("PhysicalRatioX", PhysicalRatio.X);
                    key.SetKey("PhysicalRatioY", PhysicalRatio.Y);

                    key.SetKey("PixelX", InPixel.X);
                    key.SetKey("PixelY", InPixel.Y);
                    key.SetKey("PixelWidth", InPixel.Width);
                    key.SetKey("PixelHeight", InPixel.Height);

                    key.SetKey("Primary", Primary);
                    key.SetKey("Orientation", Orientation);
                }
            }
        }

        //[TriggerOn(nameof(Monitor), "WorkArea")]
        //public AbsoluteRectangle AbsoluteWorkingArea => this.Get(() => new AbsoluteRectangle(
        //    new PixelPoint(Config, this, Monitor.WorkArea.X, Monitor.WorkArea.Y),
        //    new PixelPoint(Config, this, Monitor.WorkArea.Right, Monitor.WorkArea.Bottom)
        //));

        private static readonly List<string> SideNames = new List<string> {"Left", "Top", "Right", "Bottom"};
        private static readonly List<string> DimNames = new List<string> {"Width", "Height"};

        private double LoadRotatedValue(List<string> names, string name, string side, Func<double> def,
            bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + Orientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey() : OpenMonitorRegKey())
            {
                return key.GetKey(name.Replace("%", names[pos]), def);
            }
        }


        private void SaveRotatedValue(List<string> names, string name, string side, double value,
            bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + n - Orientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey(true) : OpenMonitorRegKey(true))
            {
                key.SetKey(name.Replace("%", names[pos]), value);
            }
        }


        [JsonProperty]
        public Rect GuiLocation
        {
            get => _guiLocation.Get();
            //{
            //    Width = 0.5,
            //    Height = (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
            //    Y = 1 - (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
            //    X = 1 - 0.5
            //});
            set => _guiLocation.Set(value);
        }
        private readonly IProperty<Rect> _guiLocation = H.Property<Rect>();

        public enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2
        } //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In] IntPtr hmonitor, [In] DpiType dpiType, [Out] out uint dpiX,
            [Out] out uint dpiY);


        private readonly IProperty<double> _winDpiX = H.Property<double>(nameof(WinDpiX), c => c
            .On(e => e.EffectiveDpi.X)
            .Set(e => e.EffectiveDpi.X)
        );
        [JsonProperty]
        public double WinDpiX => _winDpiX.Get();

        private readonly IProperty<double> _winDpiY = H.Property<double>(nameof(WinDpiY), c => c
            .On(e => e.EffectiveDpi.Y)
            .Set(e => e.EffectiveDpi.Y)
        );
        [JsonProperty]
        public double WinDpiY => _winDpiY.Get();


        [JsonProperty] public IScreenRatio RawDpi => _rawDpi.Get();
        private readonly IProperty<IScreenRatio> _rawDpi = H.Property<IScreenRatio>(c => c
            .On(e => e.Monitor.RawDpi)
            .Set(e => new ScreenRatioValue(e.Monitor.RawDpi))
        );

        [JsonProperty] public IScreenRatio EffectiveDpi => _effectiveDpi.Get();
        private readonly IProperty<IScreenRatio> _effectiveDpi = H.Property<IScreenRatio>(c => c
            .On(e => e.Monitor.EffectiveDpi)
            .Set(e => new ScreenRatioValue(e.Monitor.EffectiveDpi))
        );

        private readonly IProperty<IScreenRatio> _dpiAwareAngularDpi = H.Property<IScreenRatio>(c => c
            .On(e => e.Monitor.AngularDpi)
            .Set(e => new ScreenRatioValue(e.Monitor.AngularDpi))
        );
        [JsonProperty] public IScreenRatio DpiAwareAngularDpi => _dpiAwareAngularDpi.Get();


//        private NativeMethods.Process_DPI_Awareness DpiAwareness => this.Get(() =>
//        {
////            Process p = Process.GetCurrentProcess();

//            NativeMethods.Process_DPI_Awareness aw = NativeMethods.Process_DPI_Awareness.Per_Monitor_DPI_Aware;

//            NativeMethods.GetProcessDpiAwareness(/*p.Handle*/IntPtr.Zero, out aw);

//            return aw;
//        });


        public NativeMethods.DPI_Awareness_Context DpiAwarenessContext => _dpiAwarenessContext.Get();
        private readonly IProperty<NativeMethods.DPI_Awareness_Context> _dpiAwarenessContext 
            = H.Property<NativeMethods.DPI_Awareness_Context>(c => c
                 .Set(s => NativeMethods.GetThreadDpiAwarenessContext())
            );


        // This is the ratio used in system config

        public IScreenRatio UpdateWpfToPixelRatio()
        {
            switch (DpiAwarenessContext)
            {
                case NativeMethods.DPI_Awareness_Context.Unaware:
                    if (RealDpi == null) return null;
                    if (DpiAwareAngularDpi == null) return null;
                    return new ScreenRatioValue(
                        Math.Round((RealDpi.X / DpiAwareAngularDpi.X) * 10) / 10,
                        Math.Round((RealDpi.Y / DpiAwareAngularDpi.Y) * 10) / 10 );
                //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

                case NativeMethods.DPI_Awareness_Context.System_Aware:
                    if (Config?.PrimaryScreen == null) return new ScreenRatioValue(1, 1);
                    else return new ScreenRatioValue(
                        Config.PrimaryScreen.EffectiveDpi.X / 96,
                        Config.PrimaryScreen.EffectiveDpi.Y / 96
                    );

                case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                    return new ScreenRatioValue(
                        EffectiveDpi.X / 96,
                        EffectiveDpi.Y / 96
                    );

                case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                    return new ScreenRatioValue(
                        EffectiveDpi.X / 96,
                        EffectiveDpi.Y / 96
                        //DpiAwareAngularDpi.X / 96,
                        //DpiAwareAngularDpi.Y / 96
                    );

                case NativeMethods.DPI_Awareness_Context.Unset:
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        private readonly IProperty<IScreenRatio> _wpfToPixelRatio 
                = H.Property<IScreenRatio>(c => c.On(e => e.RealDpi.X)
            .On(e => e.RealDpi.Y)
            .On(e => e.EffectiveDpi)
            .On(e => e.DpiAwarenessContext)
            .On(e => e.Config.PrimaryScreen.EffectiveDpi.Y)
            .On(e => e.Config.MaxEffectiveDpiY)
            .Set(s => s.UpdateWpfToPixelRatio())
            );

        [JsonProperty]
        public IScreenRatio WpfToPixelRatio => _wpfToPixelRatio.Get();

        [JsonProperty]
        [TriggerOn(nameof(WpfToPixelRatio))]
        public IScreenRatio PixelToDipRatio => WpfToPixelRatio.Inverse();

        [JsonProperty]
        [TriggerOn(nameof(Pitch))]
        public IScreenRatio PhysicalToPixelRatio => Pitch.Inverse();


        [JsonProperty]
        [TriggerOn(nameof(PhysicalRatio))]
        [TriggerOn(nameof(PhysicalToPixelRatio))]
        [TriggerOn(nameof(WpfToPixelRatio))]
        public IScreenRatio MmToDipRatio => PhysicalToPixelRatio.Multiply(WpfToPixelRatio.Inverse()).Multiply(PhysicalRatio);


        [JsonProperty]
        public IScreenRatio RealDpi => _realDpi.Get();
        private readonly IProperty<IScreenRatio> _realDpi = H.Property<IScreenRatio>();

        [TriggerOn(nameof(RealPitch))]
        public void _setRealDpi()
        {
            _realDpi.Set(new ScreenRatioValue(25.4).Multiply(RealPitch.Inverse()));
        }


        [JsonProperty]
        public IScreenRatio DpiX => _dpiX.Get();
        private readonly IProperty<IScreenRatio> _dpiX = H.Property<IScreenRatio>(nameof(DpiX));
        [TriggerOn(nameof(Pitch))]
        public void _setDpiX()
        {
            _dpiX.Set(new ScreenRatioValue(25.4).Multiply(Pitch.Inverse()));
        }


        private readonly IProperty<double> _realDpiAvg = H.Property<double>(nameof(RealDpiAvg), c => c
             .On(nameof(RealDpi), "X")
            .On(nameof(RealDpi), "Y")
            .Set(s =>
            {
                if (s.RealDpi == null) return double.NaN;
                return Math.Sqrt(s.RealDpi.X * s.RealDpi.X + s.RealDpi.Y * s.RealDpi.Y) / Math.Sqrt(2);
            }));

        [JsonProperty]
        public double RealDpiAvg => _realDpiAvg.Get();

        private readonly IProperty<bool> _moving = H.Property<bool>(nameof(Moving));
        public bool Moving
        {
            get => _moving.Get();
            set
            {
                var x = XMoving;
                var y = YMoving;

                if (_moving.Set(value) || value)
                {
                    InMm.X = x;
                    InMm.Y = y;
                }
            }
        }

        private readonly IProperty<double> _xMoving = H.Property<double>(c=>c
            .On(nameof(Moving))
            .On(nameof(InMm), "X")
            .Do((e,property) =>
            {
                if(!e.Moving)
                    property.Set(e.InMm.X);
                })
        );
        public double XMoving
        {
            get => _xMoving.Get();
            set
            {
                if (Moving) _xMoving.Set(value);
                else InMmX = value;
            }
        }

        private readonly IProperty<double> _yMoving = H.Property<double>(c => c
            .On(nameof(Moving))
            .On(nameof(InMm), "Y")
            .Do((e, property) =>
            {
                if (!e.Moving)
                    property.Set(e.InMm.Y);
            })
        );
        public double YMoving
        {
            get => _yMoving.Get();
            set
            {
                if (Moving) _yMoving.Set(value);
                else InMmY = value;
            }
        }

        private double LogPixelSx
        {
            get
            {
                IntPtr hdc = NativeMethods.CreateDC("DISPLAY", Monitor.AttachedDisplay.DeviceName, null, IntPtr.Zero);
                double dpi = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.LOGPIXELSX);
                NativeMethods.DeleteDC(hdc);
                return dpi;
            }
        }


        private double RightDistance(Screen screen) =>  InMm.OutsideBounds.X - screen.InMm.OutsideBounds.Right;
        private double LeftDistance(Screen screen) => screen.InMm.OutsideBounds.X - InMm.OutsideBounds.Right;
        private double TopDistance(Screen screen) => screen.InMm.OutsideBounds.Y - InMm.OutsideBounds.Bottom;
        private double BottomDistance(Screen screen) => InMm.OutsideBounds.Y - screen.InMm.OutsideBounds.Bottom;

        private double RightDistanceToTouch(Screen screen, bool zero = false)
        {
            double top = TopDistance(screen);
            if ( top > 0 || (zero && top == 0) ) return double.PositiveInfinity;
            double bottom = BottomDistance(screen);
            if ( bottom > 0 || (zero && bottom ==0)) return double.PositiveInfinity;
            return RightDistance(screen);
        }

        double RightDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(RightDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double RightDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(RightDistance(screen), dist);
            }
            return dist;   
        }

        double LeftDistanceToTouch(Screen screen, bool zero = false)
        {
            double top = TopDistance(screen);
            if ( top > 0 || (zero && top==0)) return double.PositiveInfinity;
            double bottom = BottomDistance(screen);
            if ( bottom > 0 || (zero && bottom == 0)) return double.PositiveInfinity;
            return LeftDistance(screen);
        }
        double LeftDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(LeftDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double LeftDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(LeftDistance(screen), dist);
            }
            return dist;       
        }


        double TopDistanceToTouch(Screen screen, bool zero = false)
        {
            double left = LeftDistance(screen);
            if ( left > 0 || (zero && left==0)) return double.PositiveInfinity;
            double right = RightDistance(screen);
            if ( right > 0 || (zero && right==0)) return double.PositiveInfinity;
            return TopDistance(screen);
        }

        double TopDistanceToTouch(IEnumerable<Screen> screens, bool zero = false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(TopDistanceToTouch(screen,zero), dist);
            }
            return dist;
        }

        double TopDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(TopDistance(screen), dist);
            }
            return dist;           
        }

        double BottomDistanceToTouch(Screen screen, bool zero = false)
        {
            double left = LeftDistance(screen);
            if ( left > 0 || (zero && left==0)) return double.PositiveInfinity;
            double right = RightDistance(screen);
            if ( right > 0 || (zero && right==0)) return double.PositiveInfinity;
            return BottomDistance(screen);
        }

        double BottomDistanceToTouch(IEnumerable<Screen> screens, bool zero=false)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(BottomDistanceToTouch(screen, zero), dist);
            }
            return dist;
        }

        double BottomDistance(IEnumerable<Screen> screens)
        {
            double dist = double.PositiveInfinity;
            foreach (Screen screen in screens)
            {
                dist = Math.Min(BottomDistance(screen), dist);
            }
            return dist;           
        }


        public double HorizontalDistance(Screen screen)
        {
            double right = RightDistance(screen);
            if (right >= 0) return right;

            double left = LeftDistance(screen);
            if (left >= 0) return left;

            return Math.Max(right, left);
        }

        public double HorizontalDistance(IEnumerable<Screen> screens)
        {
            double right = RightDistanceToTouch(screens);
            if (right >= 0) return right;

            double left = LeftDistanceToTouch(screens);
            if (left >= 0) return left;

            return Math.Max(right, left);
        }

        public double VerticalDistance(IEnumerable<Screen> screens)
        {
            double top = TopDistanceToTouch(screens);
            if (top >= 0) return top;

            double bottom = BottomDistanceToTouch(screens);
            if (bottom >= 0) return bottom;

            return Math.Max(top, bottom);
        }

        public double VerticalDistance(Screen screen)
        {
            double top = TopDistance(screen);
            if (top >= 0) return top;

            double bottom = BottomDistance(screen);
            if (bottom >= 0) return bottom;

            return Math.Max(top, bottom);
        }

        public double Distance(Screen screen)
        {
            Vector v = new Vector(HorizontalDistance(screen),VerticalDistance(screen));

             if (v.X >= 0 && v.Y >= 0) return v.Length;

            if (v.X >= 0) return v.X;
            if (v.Y >= 0) return v.Y;

            return Math.Max(v.X,v.Y);
        }

        public double Distance(IEnumerable<Screen> screens)
        {
            Vector v = new Vector(HorizontalDistance(screens), VerticalDistance(screens));

            if (v.X >= 0 && v.Y >= 0) return v.Length;

            if (v.X >= 0) return v.X;
            if (v.Y >= 0) return v.Y;

            return Math.Max(v.X, v.Y);
        }

        public bool Expand(Screen s)
        {
             double moveLeft = - LeftDistance(s); if (moveLeft <= 0) return false;

            double moveRight = - RightDistance(s); if (moveRight <= 0) return false;

            double moveUp = - TopDistance(s); if (moveUp <= 0) return false;

            double moveDown = - BottomDistance(s); if (moveDown <= 0) return false;

            if (moveLeft <= moveRight && moveLeft <= moveUp && moveLeft <= moveDown)
            {
                s.InMm.X -= moveLeft;
            }
            else if (moveRight <= moveLeft && moveRight <= moveUp && moveRight <= moveDown)
            {
                s.InMm.Y += moveRight;
            }
            else if (moveUp <= moveRight && moveUp <= moveLeft && moveUp <= moveDown)
            {
                s.InMm.Y -= moveUp;
            }
            else
            {
                s.InMm.Y += moveDown;
            }
            
            return true;
        }

        public bool PhysicalOverlapWith(Screen screen)
        {
            if (InMm.X >= screen.InMm.Bounds.Right) return false;
            if (screen.InMm.X >= InMm.Bounds.Right) return false;
            if (InMm.Y >= screen.InMm.Bounds.Bottom) return false;
            if (screen.InMm.Y >= InMm.Bounds.Bottom) return false;

            return true;
        }

        public bool PhysicalTouch(Screen screen)
        {
            if (PhysicalOverlapWith(screen)) return false;
            if (InMm.X > screen.InMm.Bounds.Right) return false;
            if (screen.InMm.X > InMm.Bounds.Right) return false;
            if (InMm.Y > screen.InMm.Bounds.Bottom) return false;
            if (screen.InMm.Y > InMm.Bounds.Bottom) return false;

            return true;
        }

        public double MoveLeftToTouch(Screen screen)
        {
            if (InMm.Y >= screen.InMm.Bounds.Bottom) return -1;
            if (screen.InMm.Y >= InMm.Bounds.Bottom) return -1;
            return InMm.X - screen.InMm.Bounds.Right;
        }

        public double MoveRightToTouch(Screen screen)
        {
            if (InMm.Y >= screen.InMm.Bounds.Bottom) return -1;
            if (screen.InMm.Y >= InMm.Bounds.Bottom) return -1;
            return screen.InMm.X - InMm.Bounds.Right;
        }

        public double MoveUpToTouch(Screen screen)
        {
            if (InMm.X > screen.InMm.Bounds.Right) return -1;
            if (screen.InMm.X > InMm.Bounds.Right) return -1;
            return InMm.Y - screen.InMm.Bounds.Bottom;
        }

        public double MoveDownToTouch(Screen screen)
        {
            if (InMm.X > screen.InMm.Bounds.Right) return -1;
            if (screen.InMm.X > InMm.Bounds.Right) return -1;
            return screen.InMm.Y - InMm.Bounds.Bottom;
        }


        public void PlaceAuto(IEnumerable<Screen> screens)
        {
            double left = LeftDistanceToTouch(screens,true);
            double right = RightDistanceToTouch(screens, true);
            double top = TopDistanceToTouch(screens, true);
            double bottom = BottomDistanceToTouch(screens, true);

            if (!Config.AllowDiscontinuity && double.IsPositiveInfinity(left) && double.IsPositiveInfinity(top) && double.IsPositiveInfinity(right) && double.IsPositiveInfinity(bottom))
            {
                top = TopDistance(screens);
                right = RightDistance(screens);
                bottom = BottomDistance(screens);
                left = LeftDistance(screens);

                if (left > 0)
                {
                    if (top > 0)
                    {
                        InMm.X += LeftDistance(screens);
                        InMm.Y += TopDistance(screens);                                          
                    }
                    if (bottom > 0)
                    {
                        InMm.X += LeftDistance(screens);
                        InMm.Y -= BottomDistance(screens);
                    }
                }
                if (right > 0)
                {
                    if (top > 0)
                    {
                        InMm.X -= RightDistance(screens);
                        InMm.Y += TopDistance(screens);
                    }
                    if (bottom > 0)
                    {
                        InMm.X -= RightDistance(screens);
                        InMm.Y -= BottomDistance(screens);
                    }
                }

                left = LeftDistanceToTouch(screens,false);
                right = RightDistanceToTouch(screens, false);
                top = TopDistanceToTouch(screens, false);
                bottom = BottomDistanceToTouch(screens, false);
            }

            if (!Config.AllowDiscontinuity)
            {

                if (top > 0 && left > 0)
                {
                    if (left < top) InMm.X += left;
                    else InMm.Y += top;
                    return;
                }

                if (top > 0 && right > 0)
                {
                    if (right < top) InMm.X -= right;
                    else InMm.Y += top;
                    return;
                }

                if (bottom > 0 && right > 0)
                {
                    if (right < bottom) InMm.X -= right;
                    else InMm.Y -= bottom;
                    return;
                }

                if (bottom > 0 && left > 0)
                {
                    if (left < bottom) InMm.X += left;
                    else InMm.Y -= bottom;
                    return;
                }

                if (top < 0 && bottom < 0)
                {
                    if (left >= 0)
                    {
                        InMm.X += left;
                        return;
                    }
                    if (right >= 0)
                    {
                        InMm.X -= right;
                        return;
                    }
                }

                if (left < 0 && right < 0)
                {
                    //if (top >= 0)
                    if (top > 0)
                    {
                        InMm.Y += top;
                        return;
                    }
                    if (bottom >= 0)
                    {
                        InMm.Y -= bottom;
                        return;
                    }
                }
            }

            if (!Config.AllowOverlaps && left < 0 && right < 0 && top < 0 && bottom < 0)
            {
                if (left > right && left > top && left > bottom)
                {
                    InMm.X += left;
                }
                else if (right > top && right > bottom)
                {
                    InMm.X -= right;
                }
                else if (top > bottom)
                {
                    InMm.Y += top;
                }
                else InMm.Y -= bottom;
            }
        }


}
}

