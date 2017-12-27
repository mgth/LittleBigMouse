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
using System.Text;
using System.Windows;
using HLab.Notify;
using HLab.Windows.API;
using HLab.Windows.Monitors;
using Microsoft.Win32;

[assembly: InternalsVisibleTo("ScreenConfig")]

namespace LittleBigMouse.ScreenConfigs
{
    [DataContract]
    public class Screen : NotifierObject
    {
        public MonitorsService Service => this.Get(() => MonitorsService.D);
        internal Screen(ScreenConfig config, Monitor monitor)
        {
            Config = config;
            Monitor = monitor;
            this.SubscribeNotifier();
        }

        public Monitor Monitor
        {
            get => this.Get<Monitor>();
            private set => this.Set(value);
        }

        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>();
            private set => this.Set(value);
        }

        [TriggedOn(nameof(Config), "AllScreens", "Count")]
        public IEnumerable<Screen> OtherScreens => this.Get(() => Config.AllScreens.Where(s => !Equals(s, this)));



        public override bool Equals(object obj)
        {
            return obj is Screen other && Monitor.Equals(other.Monitor);
        }

        public override int GetHashCode()
        {
            return ("Screen" + Monitor.DeviceId).GetHashCode();
        }


        // References properties
        [TriggedOn(nameof(Monitor),"Edid" , "ManufacturerCode")]
        [TriggedOn(nameof(Monitor),"Edid", "ProductCode")]
        public string PnpCode => this.Get(() => Monitor.Edid.ManufacturerCode + Monitor.Edid.ProductCode);

        [TriggedOn(nameof(PnpCode))]
        [TriggedOn(nameof(Monitor),"Edid", "Serial")]
        public string IdMonitor => this.Get(() => PnpCode + "_" +
            // some hp monitors (at least E242) do not provide hex serial value-
            (Monitor.Edid.Serial=="01010101"?Monitor.Edid.SerialNo:Monitor.Edid.Serial));

        [TriggedOn(nameof(InPixel))]
        public string IdResolution => this.Get(() => InPixel.Width + "x" + InPixel.Height);

        [TriggedOn(nameof(IdMonitor))]
        [TriggedOn(nameof(Orientation))]
        public string Id => this.Get(() => IdMonitor + "_" + Orientation);

        [TriggedOn(nameof(Monitor),"Primary")]
        public bool Primary => this.Get(() => Monitor.Primary);

        [TriggedOn(nameof(Monitor), "AttachedDisplay", "CurrentMode", "DisplayOrientation")]
        public int Orientation => this.Get(() => Monitor.AttachedDisplay.CurrentMode.DisplayOrientation);

        public string PnpDeviceName => this.Get(() =>
        {
            var name = Html.CleanupPnpName(Monitor.DeviceString);
            using (var key = OpenConfigRegKey())
            {
                name = key.GetKey("PnpName",()=>name);
            }

            return name.ToLower() != "generic pnp monitor" ? name : Html.GetPnpName(PnpCode);
        });


        public bool Selected
        {
            get => this.Get<bool>();
            set
            {
                if (!this.Set(value)) return;
                if (!value) return;
                foreach (var screen in Config.AllBut(this)) screen.Selected = false;
            }
        }

        public bool FixedAspectRatio
        {
            get => this.Get(()=>true);
            set => this.Set(value);
        }
        public bool Placed
        {
            get => this.Get(()=>false);
            set => this.Set(value);
        }

        // Mm dimensions
        // Natives

        public double PhysicalXSystem => this.Get(()=>0);




        public ScreenSize Physical => this.Get(() => new ScreenSizeInMm(this));

        [TriggedOn(nameof(Physical))]
        [TriggedOn(nameof(Orientation))]
        public ScreenSize PhysicalRotated => this.Get(() => Physical.Rotate(Orientation));

        //Mm
        [TriggedOn(nameof(PhysicalRotated))]
        [TriggedOn(nameof(PhysicalRatio))]
        public ScreenSize InMm => this.Get(() => PhysicalRotated.Scale(PhysicalRatio));

        [TriggedOn(nameof(InMm))]
        [TriggedOn(nameof(Orientation))]
        public ScreenSize InMmUnrotated => this.Get(() => InMm.Rotate(4-Orientation));


        //Pixel
        public ScreenSize InPixel => this.Get(() => new ScreenSizeInPixels(this));

        // Dip
        [TriggedOn(nameof(InPixel))]
         public ScreenSize InDip => this.Get(() => InPixel.ScaleDip());


        [TriggedOn(nameof(PhysicalRotated), "Width")]
        [TriggedOn(nameof(PhysicalRotated), "Height")]
        [TriggedOn(nameof(InPixel), "Width")]
        [TriggedOn(nameof(InPixel), "Height")]
        //[DataMember]
        public ScreenRatio RealPitch
        {
            get => this.Get(() => new ScreenRatioValue(
                PhysicalRotated.Width / InPixel.Width,
                PhysicalRotated.Height / InPixel.Height
                ));
            set
            {
                PhysicalRotated.Width = InPixel.Width * value.X;
                PhysicalRotated.Height = InPixel.Height * value.Y;
            }
        }


        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [TriggedOn(nameof(Id))]
        public ScreenRatio PhysicalRatio => this.Get(() => new ScreenRatioValue(1,1));

        [TriggedOn(nameof(InMm), "Height")]
        [TriggedOn(nameof(InMm), "Height")]
        public double Diagonal => this.Get(() => Math.Sqrt(InMm.Width * InMm.Width + InMm.Height * InMm.Height));





        //calculated


        [TriggedOn(nameof(RealPitch))]
        [TriggedOn(nameof(PhysicalRatio))]
        public ScreenRatio Pitch => this.Get(()
            => RealPitch.Multiply(PhysicalRatio));


        [TriggedOn(nameof(Config), "AllScreens.Item.InMm.Bounds")]
        public Rect OveralBoundsWithoutThisInMm => this.Get(() =>
        {
            var r = new Rect();
            var first = true;
            foreach (var s in Config.AllBut(this))
            {
                if (first)
                {
                    r = s.InMm.Bounds;
                    first = false;
                }
                else
                    r.Union(s.InMm.Bounds);
            }
            return r;
        });







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
            return OpenMonitorRegKey(IdMonitor, create);
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

        public RegistryKey OpenConfigRegKey(bool create = false) => OpenConfigRegKey(Config.Id, IdMonitor, create);

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
                    this.Set(new Rect(new Point(left, top), new Size(width, height)), nameof(GuiLocation));
                }
            }

            using (RegistryKey key = OpenConfigRegKey(false))
            {
                if (key != null)
                {
                    InMm.X = key.GetKey("XLocationInMm", () => InMm.X, ()=>Placed=true);
                    InMm.Y = key.GetKey("YLocationInMm", () => InMm.Y, ()=>Placed=true);
                    PhysicalRatio.X = key.GetKey("PhysicalRatioX", () => PhysicalRatio.X);
                    PhysicalRatio.Y = key.GetKey("PhysicalRatioy", () => PhysicalRatio.Y);
                }
            }


        }

        public void Save(RegistryKey baseKey)
        {
            using (RegistryKey key = OpenGuiLocationRegKey(true))
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }

            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey("TopBorder",Physical.TopBorder);
                    key.SetKey("RightBorder",Physical.RightBorder);
                    key.SetKey("BottomBorder",Physical.BottomBorder);
                    key.SetKey("LeftBorder",Physical.LeftBorder);

                    if(Math.Abs(Physical.Height - Monitor.AttachedDisplay.DeviceCaps.Size.Height) > double.Epsilon)
                        key.SetKey("Height",Physical.Height);

                    if(Math.Abs(Physical.Width - Monitor.AttachedDisplay.DeviceCaps.Size.Width) > double.Epsilon)
                        key.SetKey("Whidth",Physical.Width);

                    key.SetKey("PnpName", PnpDeviceName);
                    key.SetKey("DeviceId", Monitor.DeviceId);
                }
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey("XLocationInMm", InMm.X);
                    key.SetKey("YLocationInMm", InMm.Y);
                    key.SetKey("PhysicalRatioX", PhysicalRatio.X);
                    key.SetKey("PhysicalRatioy", PhysicalRatio.Y);

                    key.SetKey("PixelX", InPixel.X);
                    key.SetKey("PixelY", InPixel.Y);
                    key.SetKey("PixelWidth", InPixel.Width);
                    key.SetKey("PixelHeight", InPixel.Height);

                    key.SetKey("Primary", Primary);
                    key.SetKey("Orientation", Orientation);
                }
            }
        }

        //[TriggedOn(nameof(Monitor), "WorkArea")]
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



        public Rect GuiLocation
        {
            get => this.Get(() => new Rect
            {
                Width = 0.5,
                Height = (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
                Y = 1 - (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
                X = 1 - 0.5
            });
            set => this.Set(value);
        }

        public enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2
        } //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In] IntPtr hmonitor, [In] DpiType dpiType, [Out] out uint dpiX,
            [Out] out uint dpiY);


        [TriggedOn(nameof(Monitor), "HMonitor")]
        public double WinDpiX => this.Get(() =>
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out var dpiX, out var dpiY);
            this.Set(dpiY, "WinDpiY");
            return dpiX;
        });

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public double WinDpiY => this.Get(()=>
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out var dpiX, out var dpiY);
            this.Set(dpiX, "WinDpiX");
            return dpiY;
        });

        public Point ScaledPoint(Point p)
        {
            NativeMethods.POINT up = new NativeMethods.POINT((int) p.X, (int) p.Y);
            NativeMethods.PhysicalToLogicalPoint(Monitor.HMonitor, ref up);
            return new Point(up.X, up.Y);
        }

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public ScreenRatio RawDpi => this.Get(() =>
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Raw, out var dpiX, out var dpiY);
            return new ScreenRatioValue(dpiX, dpiY);
        });

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public ScreenRatio EffectiveDpi => this.Get(() =>
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out var dpiX, out var dpiY);
            return new ScreenRatioValue(dpiX,dpiY);
        });

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public ScreenRatio DpiAwareAngularDpi => this.Get(() =>
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Angular, out var dpiX, out var dpiY);
            return new ScreenRatioValue(dpiX, dpiY);
        });


//        private NativeMethods.Process_DPI_Awareness DpiAwareness => this.Get(() =>
//        {
////            Process p = Process.GetCurrentProcess();

//            NativeMethods.Process_DPI_Awareness aw = NativeMethods.Process_DPI_Awareness.Per_Monitor_DPI_Aware;

//            NativeMethods.GetProcessDpiAwareness(/*p.Handle*/IntPtr.Zero, out aw);

//            return aw;
//        });

        public NativeMethods.DPI_Awareness_Context DpiAwarenessContext => this.Get(NativeMethods.GetThreadDpiAwarenessContext);

        // This is the ratio used in system config

        [TriggedOn(nameof(RealDpi),"X")]
        [TriggedOn(nameof(RealDpi),"Y")]
        [TriggedOn(nameof(DpiAwareAngularDpi))]
        [TriggedOn(nameof(EffectiveDpi))]
        [TriggedOn(nameof(DpiAwarenessContext))]
        [TriggedOn(nameof(Config), "PrimaryScreen", "EffectiveDpi","Y")]
        [TriggedOn(nameof(Config), "MaxEffectiveDpiY")]

        public ScreenRatio WpfToPixelRatio => this.Get(() =>
        {
                switch (DpiAwarenessContext)
                {
                    case NativeMethods.DPI_Awareness_Context.Unaware:
                        return new ScreenRatioValue(
                            Math.Round((RealDpi.X / DpiAwareAngularDpi.X) * 10) / 10,
                            Math.Round((RealDpi.Y / DpiAwareAngularDpi.Y) * 10) / 10
                        );;
                //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

                    case NativeMethods.DPI_Awareness_Context.System_Aware:
                            if (Config?.PrimaryScreen == null) return new ScreenRatioValue(1,1);
                            return new ScreenRatioValue(
                                Config.PrimaryScreen.EffectiveDpi.X / 96,
                                Config.PrimaryScreen.EffectiveDpi.Y / 96
                            ); ;

                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                        return new ScreenRatioValue(
                            EffectiveDpi.X / 96,
                            EffectiveDpi.Y / 96
                        ); ;

                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                        return new ScreenRatioValue(
                            EffectiveDpi.X / 96,
                            EffectiveDpi.Y / 96
                            //DpiAwareAngularDpi.X / 96,
                            //DpiAwareAngularDpi.Y / 96
                        ); ;

                default:
                        throw new ArgumentOutOfRangeException();
                }
        });

        [TriggedOn(nameof(WpfToPixelRatio))]
        public ScreenRatio PixelToDipRatio => WpfToPixelRatio.Inverse();

        [TriggedOn(nameof(Pitch))]
        public ScreenRatio PhysicalToPixelRatio => this.Get(()=>Pitch.Inverse());


        [TriggedOn(nameof(PhysicalRatio))]
        [TriggedOn(nameof(PhysicalToPixelRatio))]
        [TriggedOn(nameof(WpfToPixelRatio))]
        public ScreenRatio MmToDipRatio => this.Get(()=>PhysicalToPixelRatio.Multiply(WpfToPixelRatio.Inverse()).Multiply(PhysicalRatio));


        [TriggedOn(nameof(RealPitch))]
        public ScreenRatio RealDpi => this.Get(()=>new ScreenRatioValue(25.4).Multiply(RealPitch.Inverse()));


        [TriggedOn(nameof(Pitch))]
        public ScreenRatio DpiX => new ScreenRatioValue(25.4).Multiply(Pitch.Inverse());


        [TriggedOn(nameof(RealDpi),"X")]
        [TriggedOn(nameof(RealDpi),"Y")]
        public double RealDpiAvg => Math.Sqrt(RealDpi.X * RealDpi.X + RealDpi.Y * RealDpi.Y) / Math.Sqrt(2) ;

        public bool Moving
        {
            get => this.Get(() => false);
            set
            {
                var x = XMoving;
                var y = YMoving;

                if (!this.Set(value) || value) return;

                InMm.X = x;
                InMm.Y = y;
            }
        }

        [TriggedOn(nameof(Moving))]
        [TriggedOn(nameof(InMm), "X")]
        public double XMoving
        {
            get => this.Get(() => InMm.X);
            set
            {
                if (Moving) this.Set(value);
                else InMm.X = value;
            }
        }

        [TriggedOn(nameof(Moving))]
        [TriggedOn(nameof(InMm), "Y")]
        public double YMoving
        {
            get => this.Get(() => InMm.Y);
            set
            {
                if (Moving) this.Set(value);
                else InMm.Y = value;
            }
        }
        /*
                private double _graphicsDpiX = 0;
                private double GraphicsDpiX
                {
                    get
                    {
                        if (_graphicsDpiX != 0) return _graphicsDpiX;

                        IntPtr hdc = Gdi32.CreateDC(null, DeviceName, null, IntPtr.Zero);
                        System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                        _graphicsDpiX = gfx.DpiX;
                        gfx.Dispose();
                        Gdi32.DeleteDC(hdc);
                        return _graphicsDpiX;
                    }
                }

                private double _graphicsDpiY = 0;
                private double GraphicsDpiY
                {
                    get
                    {
                        if (_graphicsDpiY != 0) return _graphicsDpiY;

                        IntPtr hdc = Gdi32.CreateDC(null, DeviceName, null, IntPtr.Zero);
                        System.Drawing.Graphics gfx = System.Drawing.Graphics.FromHdc(hdc);
                        _graphicsDpiY = gfx.DpiY;
                        gfx.Dispose();
                        Gdi32.DeleteDC(hdc);
                        return _graphicsDpiY;
                    }
                }
                */

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

        public string CapabilitiesString
        {
            get
            {
                IntPtr hMonitor = Monitor.HMonitor; //HPhysical;

                uint len = 0;
                if (!NativeMethods.DDCCIGetCapabilitiesStringLength(hMonitor, ref len)) return "-1-";

                StringBuilder s = new StringBuilder((int)len + 1);

                if (!NativeMethods.DDCCIGetCapabilitiesString(hMonitor, s, len)) return "-2-";

                return s.ToString();
            }
        }

        double RightDistance(Screen screen) =>  InMm.OutsideBounds.X - screen.InMm.OutsideBounds.Right;
        double LeftDistance(Screen screen) => screen.InMm.OutsideBounds.X - InMm.OutsideBounds.Right;
        double TopDistance(Screen screen) => screen.InMm.OutsideBounds.Y - InMm.OutsideBounds.Bottom;
        double BottomDistance(Screen screen) => InMm.OutsideBounds.Y - screen.InMm.OutsideBounds.Bottom;

        double RightDistanceToTouch(Screen screen, bool zero = false)
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

