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
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using System.Windows;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.ScreenConfig.Dimensions;
using Microsoft.Win32;

[assembly: InternalsVisibleTo("ScreenConfig")]

namespace LittleBigMouse.ScreenConfig
{
    using H = H<Screen>;

    [DataContract]
    public class Screen : NotifierBase
    {
        [JsonIgnore] public ScreenConfig Config { get; }
        public Monitor Monitor { get; }

        public Screen(ScreenConfig config, Monitor monitor)
        {
            Config = config;
            Monitor = monitor;

            H.Initialize(this);
        }

        private readonly ITrigger _updateModel = H.Trigger(c => c
            .On(e => e.Monitor.AttachedDisplay)
            .Do(s => s.ScreenModel.Load(s.Monitor))
        );


        [JsonIgnore] public IEnumerable<Screen> OtherScreens => _otherScreens.Get();
        private readonly IProperty<IEnumerable<Screen>> _otherScreens = H.Property<IEnumerable<Screen>>(c => c
            .Set(e => e.Config.AllScreens.Where(s => !Equals(s, e)))
            .On(e => e.Config.AllScreens.Count)
            .Update()
        );

        public override bool Equals(object obj)
        {
            return obj is Screen other && Monitor.Equals(other.Monitor);
        }

        public override int GetHashCode()
        {
            return ("Screen" + Monitor.DeviceId).GetHashCode();
        }


        // References properties
        [DataMember] public string Id => _id.Get();
        private readonly IProperty<string> _id = H.Property<string>(c => c
            .Set(e => e.Monitor.IdMonitor + "_" + e.Orientation)
            .On(e => e.Monitor.IdMonitor)
            .On(e => e.Orientation)
            .Update()
        );

        [DataMember] public string IdResolution => _idResolution.Get();
        private readonly IProperty<string> _idResolution = H.Property<string>(c => c
            .Set(e => e.InPixel.Width + "x" + e.InPixel.Height)
            .On(e => e.InPixel)
            .Update()
        );

        [DataMember] public bool Primary => _primary.Get();
        private readonly IProperty<bool> _primary = H.Property<bool>(c => c
            .Set(e => e.Monitor.Primary)
            .On(e => e.Monitor.Primary)
            .Update()
        );

        [DataMember] public int Orientation => _orientation.Get();
        private readonly IProperty<int> _orientation = H.Property<int>(c => c
            .Set(s => s.Monitor.AttachedDisplay?.CurrentMode?.DisplayOrientation ?? 0)
            .On(e => e.Monitor.AttachedDisplay.CurrentMode.DisplayOrientation)
            .Update()
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
        private readonly IProperty<bool> _selected = H.Property<bool>();

        public bool Placed
        {
            get => _placed.Get();
            set => _placed.Set(value);
        }
        private readonly IProperty<bool> _placed = H.Property<bool>();

        // Mm dimensions
        // Natives

        public ScreenModel ScreenModel => _screenModel.Get();
        private readonly IProperty<ScreenModel> _screenModel = H.Property<ScreenModel>( c => c
            .Set(s => s.Config.GetScreenModel(s.Monitor.PnpCode, s.Monitor))
            .On(e => e.Monitor.PnpCode)
            .On(e => e.Monitor)
            .NotNull(e => e.Config).NotNull(e => e.Monitor.PnpCode).NotNull(e => e.Monitor)
            .Update()
        );

        [DataMember] public IScreenSize PhysicalRotated => _physicalRotated.Get();
        private readonly IProperty<IScreenSize> _physicalRotated =
            H.Property<IScreenSize>(c => c
                .Set(s => s.ScreenModel.Physical.Rotate(s.Orientation))
                .On(e => e.ScreenModel.Physical)
                .On(e => e.Orientation)
                .NotNull(e => e.ScreenModel)
                .Update()
        );


        //Mm

        [DataMember] 
        public IScreenSize InMm => _inMm.Get();
        private readonly IProperty<IScreenSize> _inMm = H.Property<IScreenSize>(c => c
            .Set(e => e.PhysicalRotated.Scale(e.PhysicalRatio).Locate())
            .On(e => e.PhysicalRotated)
            .On(e => e.PhysicalRatio)
            .Update()
        );

        [DataMember]
        public IScreenSize InMmU => _inMmU.Get();
        private readonly IProperty<IScreenSize> _inMmU = H.Property<IScreenSize>(c => c
            .Set(e => e.ScreenModel.Physical.Scale(e.PhysicalRatio))
            //            .Set(e => e.InMm.Rotate((4-e.Orientation)%4))
            .On(e => e.ScreenModel.Physical)
            .On(e => e.PhysicalRatio)
            .Update()
        );


        public double InMmX
        {
            get => _inMmX.Get();
            set
            {
                if (Primary)
                {
                    foreach (var screen in Config.AllBut(this))
                    {
                        screen.InMm.X -= value;
                    }
                }
                else
                {
                    InMm.X = value;
                }
            }
        }

        private readonly IProperty<double> _inMmX = H.Property<double>(c => c
            .Set(e => e.InMm.X)
            .On(e => e.InMm.X)
            .Update()
        );

        public double InMmY
        {
            get
            {
                _inMmU.Get();
                return _inMmY.Get();
            }
            set
            {
                if (Primary)
                {
                    foreach (var screen in Config.AllBut(this))
                    {
                        screen.InMm.Y -= value;
                        screen.InMmU.Y -= value;
                    }
                }
                else
                {
                    InMm.Y = value;
                    InMmU.Y = value;
                }
            }
        }

        private readonly IProperty<double> _inMmY = H.Property<double>(c => c
            .Set(e => e.InMm.Y)
            .On(e => e.InMm.Y)
            .Update()
        );

        private ITrigger _setUnsaved = H.Trigger(c => c
            .On(e => e.InMm.OutsideBounds)
            .On(e => e.InMm.Bounds)
            .Do(e => e.Config.Saved=false)
        );



        //Pixel
        [DataMember] public IScreenSize InPixel => _inPixel.Get();
        private readonly IProperty<IScreenSize> _inPixel = H.Property<IScreenSize>(c => c
            .Set(s => new ScreenSizeInPixels(s) as IScreenSize)
        );


        // Dip
        [DataMember] public IScreenSize InDip => _inDip.Get();
        private readonly IProperty<IScreenSize> _inDip = H.Property<IScreenSize>(c => c
            .Set(s => s.InPixel.ScaleDip(s.EffectiveDpi, s.Config))
            .On(e => e.InPixel)
            .On(e => e.EffectiveDpi)
            .On(e => e.Config)
            .Update()
        );




        [DataMember]
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
            .Set(e => new ScreenRatioValue(
                e.PhysicalRotated.Width / e.InPixel.Width,
                e.PhysicalRotated.Height / e.InPixel.Height) as IScreenRatio)
            .On(e => e.PhysicalRotated.Width)
            .On(e => e.PhysicalRotated.Height)
            .On(e => e.InPixel.Width)
            .On(e => e.InPixel.Height)
            .On(e => e.EffectiveDpi)
            .Update()
        );

        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [DataMember]
        public IScreenRatio PhysicalRatio => _physicalRatio.Get();
        private readonly IProperty<IScreenRatio> _physicalRatio = H.Property<IScreenRatio>(c=>c
                .Set(e => new ScreenRatioValue(1.0, 1.0) as IScreenRatio)
        );

        public double Diagonal => _diagonal.Get();
        private readonly IProperty<double> _diagonal = H.Property<double>(c => c
                .Set(e =>
                {
                    var w = e.InMm.Width;
                    var h = e.InMm.Height;
                    return Math.Sqrt( w*w + h*h );
                })
                .On(e => e.InMm.Height)
                .On(e => e.InMm.Width)
                .Update()
        );


        //calculated
        [DataMember]
        public IScreenRatio Pitch => _pitch.Get();
        private readonly IProperty<IScreenRatio> _pitch
            = H.Property<IScreenRatio>(c => c
                .Set(e => e.RealPitch.Multiply(e.PhysicalRatio))
                .On(e => e.RealPitch)
                .On(e => e.PhysicalRatio)
                .Update()
            );


        [DataMember] public Rect OverallBoundsWithoutThisInMm => _overallBoundsWithoutThisInMm.Get();
        private readonly IProperty<Rect> _overallBoundsWithoutThisInMm 
            = H.Property<Rect>(c => c
                .Set(e =>
                {
                    var result = new Rect();
                    var first = true;
                    foreach (var screen in e.Config.AllBut(e))
                    {
                        if (screen.InMm == null) continue; 
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
                .On(e => e.Config.AllScreens.Item().InMm.Bounds).Update()

        ); 







        // Registry settings


        public RegistryKey OpenGuiLocationRegKey(bool create = false)
        {
            using (RegistryKey key = Monitor.OpenMonitorRegKey(create))
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
            using (var key = OpenGuiLocationRegKey())
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

            using (var key = OpenConfigRegKey(false))
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

            using (var key = OpenGuiLocationRegKey(true))
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }

            using (var key = Monitor.OpenMonitorRegKey(true))
            {
                key?.SetKey("DeviceId", Monitor.DeviceId);
            }

            using (var key = OpenConfigRegKey(true))
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
            using (RegistryKey key = fromConfig ? OpenConfigRegKey() : Monitor.OpenMonitorRegKey())
            {
                return key.GetKey(name.Replace("%", names[pos]), def);
            }
        }


        private void SaveRotatedValue(List<string> names, string name, string side, double value,
            bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + n - Orientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey(true) : Monitor.OpenMonitorRegKey(true))
            {
                key.SetKey(name.Replace("%", names[pos]), value);
            }
        }


        [DataMember]
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


        [DataMember]
        public double WinDpiX => _winDpiX.Get();
        private readonly IProperty<double> _winDpiX = H.Property<double>(c => c
            .Set(e => e.EffectiveDpi.X)
            .On(e => e.EffectiveDpi.X).Update()
        );

        [DataMember]
        public double WinDpiY => _winDpiY.Get();
        private readonly IProperty<double> _winDpiY = H.Property<double>(c => c
            .Set(e => e.EffectiveDpi.Y)
            .On(e => e.EffectiveDpi.Y).Update()
        );

        [DataMember] public IScreenRatio RawDpi => _rawDpi.Get();
        private readonly IProperty<IScreenRatio> _rawDpi = H.Property<IScreenRatio>(c => c
            .Set(e => new ScreenRatioValue(e.Monitor.RawDpi) as IScreenRatio)
            .On(e => e.Monitor.RawDpi).Update()
        );

        [DataMember] public IScreenRatio EffectiveDpi => _effectiveDpi.Get();
        private readonly IProperty<IScreenRatio> _effectiveDpi = H.Property<IScreenRatio>(c => c
            .Set(e => new ScreenRatioValue(e.Monitor.EffectiveDpi) as IScreenRatio)
            .On(e => e.Monitor.EffectiveDpi).Update()
        );

        [DataMember] public IScreenRatio DpiAwareAngularDpi => _dpiAwareAngularDpi.Get();
        private readonly IProperty<IScreenRatio> _dpiAwareAngularDpi = H.Property<IScreenRatio>(c => c
            .Set(e => new ScreenRatioValue(e.Monitor.AngularDpi) as IScreenRatio)
            .On(e => e.Monitor.AngularDpi).Update()
        );


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

                case NativeMethods.DPI_Awareness_Context.StangeValue2 :
                case NativeMethods.DPI_Awareness_Context.StrangeValue :
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

        [DataMember]
        public IScreenRatio WpfToPixelRatio => _wpfToPixelRatio.Get();
        private readonly IProperty<IScreenRatio> _wpfToPixelRatio 
                = H.Property<IScreenRatio>(c => c
                    .Set(s => s.UpdateWpfToPixelRatio())
                    .On(e => e.RealDpi.X)
                    .On(e => e.RealDpi.Y)
                    .On(e => e.EffectiveDpi)
                    .On(e => e.DpiAwarenessContext)
                    .On(e => e.Config.PrimaryScreen.EffectiveDpi.Y)
                    .On(e => e.Config.MaxEffectiveDpiY)
                    .Update()
            );


        [DataMember]
        public IScreenRatio PixelToDipRatio => _pixelToDipRatio.Get();
        private readonly IProperty<IScreenRatio> _pixelToDipRatio = H.Property<IScreenRatio>(c => c
            .Set(e => e.WpfToPixelRatio.Inverse())
            .On(e => e.WpfToPixelRatio).Update()
        );


        [DataMember]
        public IScreenRatio PhysicalToPixelRatio => _physicalToPixelRatio.Get();
        private readonly IProperty<IScreenRatio> _physicalToPixelRatio = H.Property<IScreenRatio>(c => c
            .Set(e => e.Pitch.Inverse())
            .On(e => e.Pitch).Update()
        );


        [DataMember]
        public IScreenRatio MmToDipRatio => _mmToDipRatio.Get();
        private readonly IProperty<IScreenRatio> _mmToDipRatio = H.Property<IScreenRatio>(c => c
            .Set(e => e.PhysicalToPixelRatio.Multiply(e.WpfToPixelRatio.Inverse()).Multiply(e.PhysicalRatio))
            .On(e => e.PhysicalRatio)
            .On(e => e.PhysicalToPixelRatio)
            .On(e => e.WpfToPixelRatio)
            .Update()
        );

        private IScreenRatio _inch = new ScreenRatioValue(25.4);

        [DataMember]
        public IScreenRatio RealDpi => _realDpi.Get();
        private readonly IProperty<IScreenRatio> _realDpi = H.Property<IScreenRatio>(c => c
            .Set(e => e._inch.Multiply(e.RealPitch.Inverse()))
            .On(e => e.RealPitch).Update()
        );

        [DataMember]
        public IScreenRatio DpiX => _dpiX.Get();
        private readonly IProperty<IScreenRatio> _dpiX = H.Property<IScreenRatio>(c => c
            .Set(e => e._inch.Multiply(e.Pitch.Inverse()))
            .On(e => e.Pitch).Update()
        );

        [DataMember]
        public double RealDpiAvg => _realDpiAvg.Get();
        private readonly IProperty<double> _realDpiAvg = H.Property<double>(c => c
            .Set(s =>
            {
                if (s.RealDpi == null) return double.NaN;
                var x = s.RealDpi.X;
                var y = s.RealDpi.Y;
                return Math.Sqrt(x * x + y * y) / Math.Sqrt(2);
            })
            .On(e => e.RealDpi.X)
            .On(e => e.RealDpi.Y)
            .Update()
        );


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
        private readonly IProperty<bool> _moving = H.Property<bool>();

        public double XMoving
        {
            get => _xMoving.Get();
            set
            {
                if (Moving) _xMoving.Set(value);
                else InMmX = value;
            }
        }
        private readonly IProperty<double> _xMoving = H.Property<double>(c=>c
            .Set(e => e.InMm.X)
            .On(e => e.Moving)
            .On(e => e.InMm.X)
            .When(e => !e.Moving)
            .Update()
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
        private readonly IProperty<double> _yMoving = H.Property<double>(c => c
            .Set(e => e.InMm.Y)
            .On(e => e.Moving)
            .On(e => e.InMm.Y)
            .When(e => !e.Moving)
            .Update()
        );

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
            var left = LeftDistance(screen);
            if ( left > 0 || (zero && Math.Abs(left) < double.Epsilon)) return double.PositiveInfinity;
            var right = RightDistance(screen);
            if ( right > 0 || (zero && Math.Abs(right) < double.Epsilon)) return double.PositiveInfinity;
            return BottomDistance(screen);
        }

        double BottomDistanceToTouch(IEnumerable<Screen> screens, bool zero=false)
        {
            var dist = double.PositiveInfinity;
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

        private ITrigger _debug = H.Trigger(c => c
            .On(e => e.InPixel.Width)
            .Do(e => {})
        );

}
}

