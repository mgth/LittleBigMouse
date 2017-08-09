/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using Erp.Notify;
using System.Text;
using System.Windows;
using WindowsMonitors;
using WinAPI;

[assembly: InternalsVisibleTo("ScreenConfig")]

namespace LbmScreenConfig
{
    [DataContract]
    public class Screen : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public DisplayMonitor Monitor
        {
            get => this.Get<DisplayMonitor>();
            private set => this.Set(value);
        }

        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>();
            private set => this.Set(value);
        }

        [TriggedOn(nameof(Config), "AllScreens.Count")]
        public IEnumerable<Screen> OtherScreens => this.Get(() => Config.AllScreens.Where(s => !Equals(s, this)));

        internal Screen(ScreenConfig config, DisplayMonitor monitor)
        {
            using (this.Suspend())
            {
                Config = config;
                Monitor = monitor;
            }

            // Todo : XLocationInMm = OveralBoundsWithoutThisInMm.Right;
        }


        public override bool Equals(object obj)
        {
            Screen other = obj as Screen;
            if (obj == null) return base.Equals(obj);
            if (other == null) return false;
            return Monitor.Equals(other.Monitor);
        }

        public override int GetHashCode()
        {
            return ("Screen" + Monitor.DeviceId).GetHashCode();
        }

        ~Screen()
        {
        }


        // References properties
        [TriggedOn(nameof(Monitor), "ManufacturerCode")]
        [TriggedOn(nameof(Monitor), "ProductCode")]
        public string PnpCode => this.Get(() => Monitor.ManufacturerCode + Monitor.ProductCode);

        [TriggedOn(nameof(PnpCode))]
        [TriggedOn(nameof(Monitor), "Serial")]
        public string IdMonitor => this.Get(() => PnpCode + "_" + Monitor.Serial);

        [TriggedOn(nameof(SizeInPixels))]
        public string IdResolution => this.Get(() => SizeInPixels.Width + "x" + SizeInPixels.Height);

        [TriggedOn(nameof(IdMonitor))]
        [TriggedOn("Monitor.DisplayOrientation")]
        public string Id => this.Get(() => IdMonitor + "_" + Monitor.DisplayOrientation);

        [TriggedOn("Monitor.Primary")]
        public bool Primary => this.Get(() => Monitor.Primary == 1);


        public string PnpDeviceName => this.Get(() =>
        {
            var name = Html.CleanupPnpName(Monitor.DeviceString);
            using (RegistryKey key = OpenConfigRegKey())
            {
                name = key.GetKey("PnpName",()=>name);
            }

            if (name.ToLower() != "generic pnp monitor")
                return name;

            return Html.GetPnpName(PnpCode);
        });

        [TriggedOn(nameof(Monitor), "DeviceName")]
        public int DeviceNo => this.Get(() =>
        {
            var s = Monitor.DeviceName.Split('\\');
            return s.Length < 4 ? 0 : int.Parse(s[3].Replace("DISPLAY", ""));
        });

        public bool Selected
        {
            get => this.Get<bool>();
            set
            {
                if (!this.Set(value)) return;
                if (!value) return;
                foreach (Screen screen in Config.AllBut(this)) screen.Selected = false;
            }
        }

        public bool FixedAspectRatio
        {
            get => this.Get<bool>();
            set => this.Set(value);
        }

        // Mm dimensions
        // Natives

        public double PhysicalXSystem => this.Get(()=>0);


        [TriggedOn(nameof(Id))]
        [TriggedOn(nameof(Config), "Id")]
        public double PhysicalXSaved
        {
            get => this.Get(() =>
            {
                using (var key = OpenConfigRegKey())
                {
                    return key.GetKey(nameof(XLocationInMm), () => double.NaN);
                }
            });
            set
            {
                if (this.Set(value)) Config.Saved = false;
            }
        }

        [DataMember]
        [TriggedOn(nameof(PhysicalXSaved))]
        [TriggedOn(nameof(PhysicalXSystem))]
        public double XLocationInMm
        {
            get => this.Get(()=>double.IsNaN(PhysicalXSaved)?PhysicalXSystem : PhysicalXSaved);
            
            set
            {
                if (value == XLocationInMm) return;

                if (Primary)
                {
                    foreach (Screen s in Config.AllBut(this))
                    {
                        s.PhysicalXSaved = s.XLocationInMm + XLocationInMm - value;
                    }
                    PhysicalXSaved = 0.0;
                    return;
                }
                PhysicalXSaved = value;
            }
        }

        public double PhysicalYSystem => this.Get(() => 0);


        [TriggedOn(nameof(Id))]
        [TriggedOn(nameof(Config), "Id")]
        public double PhysicalYSaved
        {
            get => this.Get(() =>
            {
                using (var key = OpenConfigRegKey())
                {
                    return key.GetKey(nameof(YLocationInMm), () => double.NaN);
                }
            });
            set
            {
                if (this.Set(value)) Config.Saved = false;
            }
        }

        [DataMember]
        [TriggedOn(nameof(PhysicalYSaved))]
        [TriggedOn(nameof(PhysicalYSystem))]
        public double YLocationInMm
        {
            get => this.Get(() => double.IsNaN(PhysicalYSaved) ? PhysicalYSystem : PhysicalYSaved);

            set
            {
                if (value == YLocationInMm) return;

                if (Primary)
                {
                    foreach (Screen s in Config.AllBut(this))
                    {
                        s.PhysicalYSaved = s.YLocationInMm + YLocationInMm - value; //shift;
                    }
                    PhysicalYSaved=0.0;
                }
                PhysicalYSaved = value;
            }
        }

        [DataMember]

        [TriggedOn(nameof(Monitor), "DeviceCapsHorzSize")]
        public double RealPhysicalWidthSystem => this.Get(() => Monitor.DeviceCapsHorzSize);

        [TriggedOn(nameof(Monitor), "DeviceCapsHorzSize")]
        public double RealPhysicalWidthSaved
        {
            get => this.Get(() => LoadRotatedValue(DimNames, "Mm%", "Width", () => Double.NaN));
            set
            {
                if (this.Set(value)) Config.Saved = false;
            }
        }

        [TriggedOn(nameof(Id))]
        [TriggedOn(nameof(RealPhysicalWidthSaved))]
        [TriggedOn(nameof(RealPhysicalWidthSystem))]
        public double RealPhysicalWidth
        {
            get => this.Get(()=>double.IsNaN(RealPhysicalWidthSaved)?RealPhysicalWidthSystem:RealPhysicalWidthSaved);
            set
            {
                var ratio = value / RealPhysicalWidth;

                RealPhysicalWidthSaved = value == RealPhysicalWidthSystem ? double.NaN : value;

                if (FixedAspectRatio)
                {
                    FixedAspectRatio = false;
                    RealPhysicalHeight = RealPhysicalHeight * ratio;
                    FixedAspectRatio = true;
                }
            }
        }

        [TriggedOn(nameof(Monitor),"DisplayOrientation")]
        [TriggedOn(nameof(RealPhysicalWidth))]
        [TriggedOn(nameof(RealPhysicalHeight))]
        public double UnrotatedRealPhysicalWidth
        {
            get => this.Get(()=>Monitor.DisplayOrientation % 2 == 0 ? RealPhysicalWidth : RealPhysicalHeight);
            set
            {
                if (Monitor.DisplayOrientation % 2 == 0)
                    RealPhysicalWidth = value;
                else
                    RealPhysicalHeight = value;
            }
        }

        [TriggedOn(nameof(Monitor), "DisplayOrientation")]
        [TriggedOn(nameof(RealPhysicalHeight))]
        [TriggedOn(nameof(RealPhysicalWidth))]

        public double UnrotatedRealPhysicalHeight
        {
            get => (Monitor.DisplayOrientation % 2 == 0) ? RealPhysicalHeight : RealPhysicalWidth;
            set
            {
                if (Monitor.DisplayOrientation % 2 == 0)
                    RealPhysicalHeight = value;
                else 
                    RealPhysicalWidth = value;
            }
        }

        [TriggedOn(nameof(Monitor),"DeviceCapsVertSize")]
        public double RealPhysicalHeightSystem => this.Get(() => Monitor?.DeviceCapsVertSize ?? 0);

        public double RealPhysicalHeightSaved
        {
            get => this.Get(() => LoadRotatedValue(DimNames, "Mm%", "Height", () => double.NaN));
            set
            {
                if (this.Set(value)) Config.Saved = false;
            }
        }

        [DataMember]
        [TriggedOn(nameof(Id))]
        [TriggedOn(nameof(RealPhysicalHeightSaved))]
        [TriggedOn(nameof(RealPhysicalHeightSystem))]
        public double RealPhysicalHeight
        {
            get => this.Get(() => double.IsNaN(RealPhysicalHeightSaved)? RealPhysicalHeightSystem:RealPhysicalHeightSaved);
            set
            {
                double ratio = value / RealPhysicalHeight;
                RealPhysicalHeightSaved = value;

                if (FixedAspectRatio)
                {
                    FixedAspectRatio = false;
                    RealPhysicalWidth = value * ratio;
                    FixedAspectRatio = true;
                }
            }
        }


        [TriggedOn(nameof(RealPhysicalWidth))]
        [TriggedOn(nameof(SizeInPixels))]
        //[DataMember]
        public double RealPitchX
        {
            get => this.Get(() => RealPhysicalWidth / SizeInPixels.Width);
            set => RealPhysicalWidth = SizeInPixels.Width * value;
        }

        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [TriggedOn(nameof(RealPhysicalHeight))]
        [TriggedOn(nameof(SizeInPixels))]
        public double RealPitchY
        {
            get => this.Get(() => RealPhysicalHeight / SizeInPixels.Height);
            set => RealPhysicalHeight = SizeInPixels.Height * value;
        }

        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [TriggedOn(nameof(Id))]
        public double PhysicalRatioX
        {
            get
            {
                var value = this.Get(() => double.NaN);
                return double.IsNaN(value) ? 1 : value;
            }
            set => this.Set((value == 1) ? double.NaN : value);
        }

        /// <summary>
        /// Final ratio to deal with screen distance
        /// </summary>
        [TriggedOn(nameof(Id))]
        public double PhysicalRatioY
        {
            get
            {
                var value = this.Get(() => double.NaN);
                return double.IsNaN(value) ? 1 : value;
            }
            set => this.Set((value == 1) ? double.NaN : value);
        }


        //calculated
        [TriggedOn(nameof(PhysicalRatioX))]
        [TriggedOn(nameof(RealPhysicalWidth))]
        public double WidthInMm => this.Get(()
            => PhysicalRatioX * RealPhysicalWidth);

        [TriggedOn(nameof(PhysicalRatioY))]
        [TriggedOn(nameof(RealPhysicalHeight))]
        public double HeightInMm => this.Get(()
            => PhysicalRatioY * RealPhysicalHeight);

        [TriggedOn(nameof(RealPitchX))]
        [TriggedOn(nameof(PhysicalRatioX))]
        public double PitchX => this.Get(()
            => RealPitchX * PhysicalRatioX);

        [TriggedOn(nameof(RealPitchY))]
        [TriggedOn(nameof(PhysicalRatioY))]
        public double PitchY => this.Get(()
            => RealPitchY * PhysicalRatioY);

        [TriggedOn(nameof(XLocationInMm))]
        [TriggedOn(nameof(YLocationInMm))]
        public Point LocationInMm
        {
            get => this.Get(() => new Point(XLocationInMm, YLocationInMm));
            set
            {
                using (this.Suspend())
                {
                    XLocationInMm = value.X;
                    YLocationInMm = value.Y;
                }
            }
        }

        /// <summary>
        /// Mm screen size in mm
        /// (display area without borders)
        /// </summary>
        //[TriggedOn(nameof(Monitor))]
        [TriggedOn(nameof(WidthInMm))]
        [TriggedOn(nameof(HeightInMm))]
        public Size SizeInMm => this.Get(() => new Size(WidthInMm, HeightInMm));

        /// <summary>
        /// Mm screen bounds
        /// </summary>
        [TriggedOn(nameof(LocationInMm))]
        [TriggedOn(nameof(SizeInMm))]
        public Rect BoundsInMm => this.Get(() => new Rect(
            LocationInMm,
            SizeInMm
        ));

        [TriggedOn(nameof(Config), "AllScreens.Item.BoundsInMm")]
        public Rect OveralBoundsWithoutThisInMm => this.Get(() =>
        {
            var r = new Rect();
            var first = true;
            foreach (var s in Config.AllBut(this))
            {
                if (first)
                {
                    r = s.BoundsInMm;
                    first = false;
                }
                else
                    r.Union(s.BoundsInMm);
            }
            return r;
        });


        // Pixel native Dimensions 
        /// <summary>
        /// Screen bounds in pixels
        /// </summary>
        [TriggedOn(nameof(Monitor), "MonitorArea")]
        public Rect BoundsInPixels => this.Get(() => Monitor.MonitorArea);

        /// <summary>
        /// Screen size in pixels
        /// </summary>
        [TriggedOn(nameof(BoundsInPixels))]
        public Size SizeInPixels => this.Get(() => BoundsInPixels.Size);

        [TriggedOn(nameof(LocationInPixels))]
        [TriggedOn(nameof(SizeInPixels))]
        public AbsolutePoint BottomRight => this.Get(()
            => new PixelPoint(Config, this, LocationInPixels.X + SizeInPixels.Width, LocationInPixels.Y + SizeInPixels.Height));

        [TriggedOn(nameof(BoundsInPixels))]
        [TriggedOn(nameof(BottomRight))]
        public AbsoluteRectangle Bounds => this.Get(()
            => new AbsoluteRectangle(new PixelPoint(Config, this, BoundsInPixels.X, BoundsInPixels.Y).Inside, BottomRight.Pixel.Inside));

        /// <summary>
        /// Screen location in pixels
        /// (this is Windows location)
        /// </summary>
        [TriggedOn(nameof(BoundsInPixels))]
        public PixelPoint LocationInPixels => this.Get(()
            => new PixelPoint(Config, this, BoundsInPixels.X, BoundsInPixels.Y));

        // Dip
        [TriggedOn(nameof(BoundsInPixels))]
        [TriggedOn(nameof(PixelToDipRatioX))]
        public double WidthInDip => this.Get(() 
            => BoundsInPixels.Width * PixelToDipRatioX);

        [TriggedOn(nameof(BoundsInPixels))]
        [TriggedOn(nameof(PixelToDipRatioY))]
        public double HeightInDip => this.Get(()
            => BoundsInPixels.Height * PixelToDipRatioY);



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
            if (create) System.IO.Directory.CreateDirectory(path);

            return path;
        }

        [TriggedOn("Config.Id")]
        void UpdateConfigId()
        {
            PhysicalRatioX = double.NaN;
            PhysicalRatioY = double.NaN;
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
                    SaveRotatedValue(SideNames, "%Border", "Left", this.Get<double>(nameof(LeftBorderInMm)));
                    SaveRotatedValue(SideNames, "%Border", "Top", this.Get<double>(nameof(TopBorderInMm)));
                    SaveRotatedValue(SideNames, "%Border", "Right", this.Get<double>(nameof(RightBorderInMm)));
                    SaveRotatedValue(SideNames, "%Border", "Bottom", this.Get<double>(nameof(BottomBorderInMm)));

                    SaveRotatedValue(DimNames, "Mm%", "Width", this.Get<double>(nameof(RealPhysicalWidthSaved)));
                    SaveRotatedValue(DimNames, "Mm%", "Height", this.Get<double>(nameof(RealPhysicalHeightSaved)));

                    key.SetKey("PnpName", PnpDeviceName);
                    key.SetKey("DeviceId", Monitor.DeviceId);
                }
            }

            using (RegistryKey key = OpenConfigRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey(nameof(XLocationInMm), this.Get<double>(nameof(PhysicalXSaved)));
                    key.SetKey(nameof(YLocationInMm), this.Get<double>(nameof(PhysicalYSaved)));
                    key.SetKey(nameof(PhysicalRatioX), this.Get<double>(nameof(PhysicalRatioX)));
                    key.SetKey(nameof(PhysicalRatioY), this.Get<double>(nameof(PhysicalRatioY)));

                    key.SetKey("PixelX", LocationInPixels.X);
                    key.SetKey("PixelY", LocationInPixels.Y);
                    key.SetKey("PixelWidth", SizeInPixels.Width);
                    key.SetKey("PixelHeight", SizeInPixels.Width);

                    key.SetKey("Primary", Primary);
                    key.SetKey("Orientation", Monitor.DisplayOrientation);
                }
            }
        }

        [TriggedOn(nameof(Monitor), "WorkArea")]
        public AbsoluteRectangle AbsoluteWorkingArea => this.Get(() => new AbsoluteRectangle(
            new PixelPoint(Config, this, Monitor.WorkArea.X, Monitor.WorkArea.Y),
            new PixelPoint(Config, this, Monitor.WorkArea.Right, Monitor.WorkArea.Bottom)
        ));

        private static readonly List<string> SideNames = new List<string> {"Left", "Top", "Right", "Bottom"};
        private static readonly List<string> DimNames = new List<string> {"Width", "Height"};

        private double LoadRotatedValue(List<string> names, string name, string side, Func<double> def,
            bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + Monitor.DisplayOrientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey() : OpenMonitorRegKey())
            {
                return key.GetKey(name.Replace("%", names[pos]), def);
            }
        }


        private void SaveRotatedValue(List<string> names, string name, string side, double value,
            bool fromConfig = false)
        {
            int n = names.Count;
            int pos = (names.IndexOf(side) + n - Monitor.DisplayOrientation) % n;
            using (RegistryKey key = fromConfig ? OpenConfigRegKey(true) : OpenMonitorRegKey(true))
            {
                key.SetKey(name.Replace("%", names[pos]), value);
            }
        }


        [TriggedOn("Id")]
        public double LeftBorderInMm
        {
            get => this.Get(() => LoadRotatedValue(SideNames, "%Border", "Left", ()=>20.0));
            set => this.Set((value < 0) ? 0 : value);
        }

        [TriggedOn("Id")]
        public double RightBorderInMm
        {
            get => this.Get(() => LoadRotatedValue(SideNames, "%Border", "Right", ()=>20.0));
            set => this.Set((value < 0) ? 0 : value);
        }

        [TriggedOn("Id")]
        public double TopBorderInMm
        {
            get => this.Get(() => LoadRotatedValue(SideNames, "%Border", "Top", ()=>20.0));
            set => this.Set((value < 0) ? 0 : value);
        }

        [TriggedOn("Id")]
        public double BottomBorderInMm
        {
            get => this.Get(() => LoadRotatedValue(SideNames, "%Border", "Bottom", ()=>20.0));
            set => this.Set((value < 0) ? 0 : value);
        }

        [TriggedOn(nameof(LeftBorderInMm))]
        [TriggedOn(nameof(PhysicalRatioX))]
        public double LeftBorder => this.Get(() => LeftBorderInMm * PhysicalRatioX);

        [TriggedOn(nameof(RightBorderInMm))]
        [TriggedOn(nameof(PhysicalRatioX))]
        public double RightBorder => this.Get(() => RightBorderInMm * PhysicalRatioX);

        [TriggedOn(nameof(TopBorderInMm))]
        [TriggedOn(nameof(PhysicalRatioY))]
        public double TopBorder => this.Get(() => TopBorderInMm * PhysicalRatioY);

        [TriggedOn(nameof(BottomBorderInMm))]
        [TriggedOn(nameof(PhysicalRatioY))]
        public double BottomBorder => this.Get(() => BottomBorderInMm * PhysicalRatioY);


        [TriggedOn(nameof(XLocationInMm))]
        [TriggedOn(nameof(YLocationInMm))]
        [TriggedOn(nameof(LeftBorder))]
        [TriggedOn(nameof(TopBorder))]
        [TriggedOn(nameof(RightBorder))]
        [TriggedOn(nameof(BottomBorder))]
        [TriggedOn(nameof(WidthInMm))]
        [TriggedOn(nameof(HeightInMm))]
        public Rect OutsideBoundsInMm
        {
            get => this.Get(() =>
            {
                double x = XLocationInMm - LeftBorder;
                double y = YLocationInMm - TopBorder;
                double w = WidthInMm + LeftBorder + RightBorder;
                double h = HeightInMm + TopBorder + BottomBorder;
                return new Rect(new Point(x, y), new Size(w, h));
            });
        }


        public Rect GuiLocation
        {
            get => this.Get(() => new Rect
            {
                Width = 0.5,
                Height = (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
                Y = 1 - (9 * Monitor.WorkArea.Width / 16) / Monitor.WorkArea.Height,
                X = 1 - 0.5,
            });
            set => this.Set(value);
        }

        public enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        } //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In] IntPtr hmonitor, [In] DpiType dpiType, [Out] out uint dpiX,
            [Out] out uint dpiY);


        private void GetWinDpi()
        {
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out _winDpiX, out _winDpiY);
        }

        private uint _winDpiX = 0;

        public double WinDpiX
        {
            get
            {
                if (_winDpiX == 0) GetWinDpi();
                return _winDpiX;
            }
        }

        private uint _winDpiY = 0;

        public double WinDpiY
        {
            get
            {
                if (_winDpiY == 0) GetWinDpi();
                return _winDpiY;
            }
        }

        public Point ScaledPoint(Point p)
        {
            NativeMethods.POINT up = new NativeMethods.POINT((int) p.X, (int) p.Y);
            NativeMethods.PhysicalToLogicalPoint(Monitor.HMonitor, ref up);
            return new Point(up.X, up.Y);
        }

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public double RawDpiX => this.Get(() =>
        {
            uint dpiX;
            uint dpiY;
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Raw, out dpiX, out dpiY);
            return dpiX;
        });

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public double RawDpiY => this.Get(()=>
        { 
            uint dpiX;
            uint dpiY;

            GetDpiForMonitor(Monitor.HMonitor, DpiType.Raw, out dpiX, out dpiY);
            return dpiY;
        });

        [TriggedOn(nameof(Monitor), "HMonitor")]
        public double EffectiveDpiX => this.Get(() =>
        {
            uint dpiX;
            uint dpiY;
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out dpiX, out dpiY);
            return dpiX;
        });

        public double EffectiveDpiY => this.Get(() =>
        {
            uint dpiX;
            uint dpiY;
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Effective, out dpiX, out dpiY);
            return dpiY;
        });

        public double DpiAwareAngularDpiX => this.Get(() =>
        {
            uint dpiX;
            uint dpiY;
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Angular, out dpiX, out dpiY);
            return dpiX;
        });

        public double DpiAwareAngularDpiY => this.Get(() =>
        {
            uint dpiX;
            uint dpiY;
            GetDpiForMonitor(Monitor.HMonitor, DpiType.Angular, out dpiX, out dpiY);
            return dpiY;
        });

//        private NativeMethods.Process_DPI_Awareness DpiAwareness => this.Get(() =>
//        {
////            Process p = Process.GetCurrentProcess();

//            NativeMethods.Process_DPI_Awareness aw = NativeMethods.Process_DPI_Awareness.Per_Monitor_DPI_Aware;

//            NativeMethods.GetProcessDpiAwareness(/*p.Handle*/IntPtr.Zero, out aw);

//            return aw;
//        });

        private NativeMethods.DPI_Awareness_Context DpiAwarenessContext => this.Get(NativeMethods.GetThreadDpiAwarenessContext);

        // This is the ratio used in system config
        [TriggedOn(nameof(RealDpiX))]
        [TriggedOn(nameof(DpiAwareAngularDpiX))]
        [TriggedOn(nameof(EffectiveDpiX))]
        [TriggedOn(nameof(Config), "MaxEffectiveDpiX")]
        [TriggedOn(nameof(Config), "PrimaryScreen.EffectiveDpiX")]
        [TriggedOn(nameof(DpiAwarenessContext))]
        public double WpfToPixelRatioX => this.Get(() =>
        {
            switch (DpiAwarenessContext)
            {
                case NativeMethods.DPI_Awareness_Context.Unaware:
                    //return (RealDpiX / DpiAwareAngularDpiX);
                    return Math.Round((RealDpiX / DpiAwareAngularDpiX) * 10) / 10;
                case NativeMethods.DPI_Awareness_Context.System_Aware:
                    if (Config?.PrimaryScreen == null) return 1;
                    return Config.PrimaryScreen.EffectiveDpiX / 96;
                case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                    return EffectiveDpiX / 96;
                    //return Config.MaxEffectiveDpiX / 96;
                case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                    return EffectiveDpiX / 96;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        });

        [TriggedOn(nameof(RealDpiY))]
        [TriggedOn(nameof(DpiAwareAngularDpiY))]
        [TriggedOn(nameof(EffectiveDpiY))]
        [TriggedOn(nameof(DpiAwarenessContext))]
        [TriggedOn(nameof(Config), "PrimaryScreen.EffectiveDpiY")]
        [TriggedOn(nameof(Config), "MaxEffectiveDpiY")]

        public double WpfToPixelRatioY => this.Get(() =>
        {
                switch (DpiAwarenessContext)
                {
                    case NativeMethods.DPI_Awareness_Context.Unaware:
                        return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 10) / 10;
                        //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;
                    case NativeMethods.DPI_Awareness_Context.System_Aware:
                        if (Config?.PrimaryScreen == null) return 1;
                        return Config.PrimaryScreen.EffectiveDpiY / 96;
                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                        return EffectiveDpiY / 96;
                        //return 2 * Config.MaxEffectiveDpiX / 96;
                    case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                        return EffectiveDpiY / 96;
                //return Config.MaxEffectiveDpiY / 96;
                default:
                        throw new ArgumentOutOfRangeException();
                }
        });

        [TriggedOn(nameof(WpfToPixelRatioX))]
        public double PixelToDipRatioX => 1 / WpfToPixelRatioX;

        [TriggedOn(nameof(WpfToPixelRatioY))]
        public double PixelToDipRatioY => 1 / WpfToPixelRatioY;

        [TriggedOn(nameof(PitchX))]
        public double PhysicalToPixelRatioX => 1 / PitchX;

        [TriggedOn(nameof(PitchY))]
        public double PhysicalToPixelRatioY => 1 / PitchY;


        [TriggedOn(nameof(PhysicalToPixelRatioX))]
        [TriggedOn(nameof(WpfToPixelRatioX))]
        public double MmToDipRatioX => this.Get(()=>PhysicalToPixelRatioX / WpfToPixelRatioX);

        [TriggedOn(nameof(PhysicalToPixelRatioY))]
        [TriggedOn(nameof(WpfToPixelRatioY))]
        public double MmToDipRatioY => this.Get(()=>PhysicalToPixelRatioY / WpfToPixelRatioY);

        [TriggedOn(nameof(RealPitchX))]
        public double RealDpiX
        {
            get => 25.4 / RealPitchX;
            set => RealPitchX = 25.4 / value; 
        }

        [TriggedOn(nameof(RealPitchY))]
        public double RealDpiY
        {
            set => RealPitchY = 25.4 / value; get => 25.4 / RealPitchY;
        }

        [TriggedOn(nameof(PitchX))]
        public double DpiX => 25.4 / PitchX;

        [TriggedOn(nameof(PitchY))]
        public double DpiY => 25.4 / PitchY;

        [TriggedOn(nameof(PitchX))]
        [TriggedOn(nameof(PitchY))]
        public double RealDpiAvg
        {
            get
            {
                double pitch = Math.Sqrt((PitchX * PitchX) + (PitchY * PitchY)) / Math.Sqrt(2);
                return 25.4 / pitch;
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
                IntPtr hdc = NativeMethods.CreateDC("DISPLAY", Monitor.Adapter.DeviceName, null, IntPtr.Zero);
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

        double RightDistance(Screen screen) =>  OutsideBoundsInMm.X - screen.OutsideBoundsInMm.Right;
        double LeftDistance(Screen screen) => screen.OutsideBoundsInMm.X - OutsideBoundsInMm.Right;
        double TopDistance(Screen screen) => screen.OutsideBoundsInMm.Y - OutsideBoundsInMm.Bottom;
        double BottomDistance(Screen screen) => OutsideBoundsInMm.Y - screen.OutsideBoundsInMm.Bottom;

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
                s.XLocationInMm -= moveLeft;
            }
            else if (moveRight <= moveLeft && moveRight <= moveUp && moveRight <= moveDown)
            {
                s.XLocationInMm += moveRight;
            }
            else if (moveUp <= moveRight && moveUp <= moveLeft && moveUp <= moveDown)
            {
                s.YLocationInMm -= moveUp;
            }
            else
            {
                s.YLocationInMm += moveDown;
            }
            
            return true;
        }

        public bool PhysicalOverlapWith(Screen screen)
        {
            if (XLocationInMm >= screen.BoundsInMm.Right) return false;
            if (screen.XLocationInMm >= BoundsInMm.Right) return false;
            if (YLocationInMm >= screen.BoundsInMm.Bottom) return false;
            if (screen.YLocationInMm >= BoundsInMm.Bottom) return false;

            return true;
        }

        public bool PhysicalTouch(Screen screen)
        {
            if (PhysicalOverlapWith(screen)) return false;
            if (XLocationInMm > screen.BoundsInMm.Right) return false;
            if (screen.XLocationInMm > BoundsInMm.Right) return false;
            if (YLocationInMm > screen.BoundsInMm.Bottom) return false;
            if (screen.YLocationInMm > BoundsInMm.Bottom) return false;

            return true;
        }

        public double MoveLeftToTouch(Screen screen)
        {
            if (YLocationInMm >= screen.BoundsInMm.Bottom) return -1;
            if (screen.YLocationInMm >= BoundsInMm.Bottom) return -1;
            return XLocationInMm - screen.BoundsInMm.Right;
        }

        public double MoveRightToTouch(Screen screen)
        {
            if (YLocationInMm >= screen.BoundsInMm.Bottom) return -1;
            if (screen.YLocationInMm >= BoundsInMm.Bottom) return -1;
            return screen.XLocationInMm - BoundsInMm.Right;
        }

        public double MoveUpToTouch(Screen screen)
        {
            if (XLocationInMm > screen.BoundsInMm.Right) return -1;
            if (screen.XLocationInMm > BoundsInMm.Right) return -1;
            return YLocationInMm - screen.BoundsInMm.Bottom;
        }

        public double MoveDownToTouch(Screen screen)
        {
            if (XLocationInMm > screen.BoundsInMm.Right) return -1;
            if (screen.XLocationInMm > BoundsInMm.Right) return -1;
            return screen.YLocationInMm - BoundsInMm.Bottom;
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
                        XLocationInMm += LeftDistance(screens);
                        YLocationInMm += TopDistance(screens);                                          
                    }
                    if (bottom > 0)
                    {
                        XLocationInMm += LeftDistance(screens);
                        YLocationInMm -= BottomDistance(screens);
                    }
                }
                if (right > 0)
                {
                    if (top > 0)
                    {
                        XLocationInMm -= RightDistance(screens);
                        YLocationInMm += TopDistance(screens);
                    }
                    if (bottom > 0)
                    {
                        XLocationInMm -= RightDistance(screens);
                        YLocationInMm -= BottomDistance(screens);
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
                    if (left < top) XLocationInMm += left;
                    else YLocationInMm += top;
                    return;
                }

                if (top > 0 && right > 0)
                {
                    if (right < top) XLocationInMm -= right;
                    else YLocationInMm += top;
                    return;
                }

                if (bottom > 0 && right > 0)
                {
                    if (right < bottom) XLocationInMm -= right;
                    else YLocationInMm -= bottom;
                    return;
                }

                if (bottom > 0 && left > 0)
                {
                    if (left < bottom) XLocationInMm += left;
                    else YLocationInMm -= bottom;
                    return;
                }

                if (top < 0 && bottom < 0)
                {
                    if (left >= 0)
                    {
                        XLocationInMm += left;
                        return;
                    }
                    if (right >= 0)
                    {
                        XLocationInMm -= right;
                        return;
                    }
                }

                if (left < 0 && right < 0)
                {
                    //if (top >= 0)
                    if (top > 0)
                    {
                        YLocationInMm += top;
                        return;
                    }
                    if (bottom >= 0)
                    {
                        YLocationInMm -= bottom;
                        return;
                    }
                }
            }

            if (!Config.AllowOverlaps && left < 0 && right < 0 && top < 0 && bottom < 0)
            {
                if (left > right && left > top && left > bottom)
                {
                    XLocationInMm += left;
                }
                else if (right > top && right > bottom)
                {
                    XLocationInMm -= right;
                }
                else if (top > bottom)
                {
                    YLocationInMm += top;
                }
                else YLocationInMm -= bottom;
            }
        }
    }
}

