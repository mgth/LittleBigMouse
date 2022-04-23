using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;

using System;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Windows;

namespace LittleBigMouse.DisplayLayout;
using H = H<MonitorSource>;

public class MonitorSource : NotifierBase
{
    public Monitor Monitor { get; }
    public MonitorDevice Device { get; }

    public MonitorSource(Monitor monitor, MonitorDevice device)
    {
        Monitor = monitor;
        Device = device;

        H.Initialize(this);
    }

    [DataMember] public string IdResolution => _idResolution.Get();
    private readonly IProperty<string> _idResolution = H.Property<string>(c => c
        .Set(e => e.InPixel.Width + "x" + e.InPixel.Height)
        .On(e => e.InPixel)
        .Update()
    );

    [DataMember]
    public bool Primary => _primary.Get();
    private readonly IProperty<bool> _primary = H.Property<bool>(c => c
        .Set(e => e.Device.Primary)
        .On(e => e.Device.Primary)
        .Update()
    );

    //Pixel
    [DataMember] public IDisplaySize InPixel => _inPixel.Get();
    private readonly IProperty<IDisplaySize> _inPixel = H.Property<IDisplaySize>(c => c
        .Set(s => new ScreenSizeInPixels(s) as IDisplaySize)
    );

    // Dip
    [DataMember] public IDisplaySize InDip => _inDip.Get();
    private readonly IProperty<IDisplaySize> _inDip = H.Property<IDisplaySize>(c => c
        .Set(s => s.InPixel.ScaleDip(s.EffectiveDpi, s.Monitor.Layout))
        .On(e => e.InPixel)
        .On(e => e.EffectiveDpi)
        .On(e => e.Monitor.Layout)
        .Update()
    );

    [DataMember]
    public IDisplayRatio RealPitch
    {
        get => _realPitch.Get();
        set
        {
            Monitor.PhysicalRotated.Width = InPixel.Width * value.X;
            Monitor.PhysicalRotated.Height = InPixel.Height * value.Y;
        }
    }
    private readonly IProperty<IDisplayRatio> _realPitch = H.Property<IDisplayRatio>(c => c
        .Set(e => new DisplayRatioValue(
            e.Monitor.PhysicalRotated.Width / e.InPixel.Width,
            e.Monitor.PhysicalRotated.Height / e.InPixel.Height) as IDisplayRatio)
        .On(e => e.Monitor.PhysicalRotated.Width)
        .On(e => e.Monitor.PhysicalRotated.Height)
        .On(e => e.InPixel.Width)
        .On(e => e.InPixel.Height)
        .On(e => e.EffectiveDpi)
        .Update()
    );

    //calculated
    [DataMember]
    public IDisplayRatio Pitch => _pitch.Get();
    private readonly IProperty<IDisplayRatio> _pitch
        = H.Property<IDisplayRatio>(c => c
            .Set(e => e.RealPitch.Multiply(e.Monitor.PhysicalRatio))
            .On(e => e.RealPitch)
            .On(e => e.Monitor.PhysicalRatio)
            .Update()
        );

    [DataMember]
    public IDisplayRatio PixelToDipRatio => _pixelToDipRatio.Get();
    private readonly IProperty<IDisplayRatio> _pixelToDipRatio = H.Property<IDisplayRatio>(c => c
        .Set(e => e.WpfToPixelRatio.Inverse())
        .On(e => e.WpfToPixelRatio).Update()
    );

    [DataMember]
    public IDisplayRatio WpfToPixelRatio => _wpfToPixelRatio.Get();
    private readonly IProperty<IDisplayRatio> _wpfToPixelRatio
            = H.Property<IDisplayRatio>(c => c
                .Set(s => s.UpdateWpfToPixelRatio())
                .On(e => e.RealDpi.X)
                .On(e => e.RealDpi.Y)
                .On(e => e.EffectiveDpi)
                .On(e => e.Monitor.Layout.DpiAwarenessContext)
                .On(e => e.Monitor.Layout.PrimarySource.EffectiveDpi.Y)
                .On(e => e.Monitor.Layout.MaxEffectiveDpiY)
                .Update()
        );



    [DataMember]
    public IDisplayRatio MmToDipRatio => _mmToDipRatio.Get();
    private readonly IProperty<IDisplayRatio> _mmToDipRatio = H.Property<IDisplayRatio>(c => c
        .Set(e => e.PhysicalToPixelRatio.Multiply(e.WpfToPixelRatio.Inverse()).Multiply(e.Monitor.PhysicalRatio))
        .On(e => e.Monitor.PhysicalRatio)
        .On(e => e.PhysicalToPixelRatio)
        .On(e => e.WpfToPixelRatio)
        .Update()
    );


    [DataMember]
    public IDisplayRatio RealDpi => _realDpi.Get();
    private readonly IProperty<IDisplayRatio> _realDpi = H.Property<IDisplayRatio>(c => c
        .Set(e => e._inch.Multiply(e.RealPitch.Inverse()))
        .On(e => e.RealPitch).Update()
    );

    [DataMember]
    public IDisplayRatio DpiX => _dpiX.Get();
    private readonly IProperty<IDisplayRatio> _dpiX = H.Property<IDisplayRatio>(c => c
        .Set(e => e._inch.Multiply(e.Pitch.Inverse()))
        .On(e => e.Pitch).Update()
    );

    private IDisplayRatio _inch = new DisplayRatioValue(25.4);

    [DataMember]
    public IDisplayRatio PhysicalToPixelRatio => _physicalToPixelRatio.Get();
    private readonly IProperty<IDisplayRatio> _physicalToPixelRatio = H.Property<IDisplayRatio>(c => c
        .Set(e => e.Pitch.Inverse())
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

    [DataMember] public IDisplayRatio RawDpi => _rawDpi.Get();
    private readonly IProperty<IDisplayRatio> _rawDpi = H.Property<IDisplayRatio>(c => c
        .Set(e => new DisplayRatioValue(e.Device.RawDpi) as IDisplayRatio)
        .On(e => e.Device.RawDpi).Update()
    );

    [DataMember] public IDisplayRatio EffectiveDpi => _effectiveDpi.Get();
    private readonly IProperty<IDisplayRatio> _effectiveDpi = H.Property<IDisplayRatio>(c => c
        .Set(e => new DisplayRatioValue(e.Device.EffectiveDpi) as IDisplayRatio)
        .On(e => e.Device.EffectiveDpi).Update()
    );

    [DataMember] public IDisplayRatio DpiAwareAngularDpi => _dpiAwareAngularDpi.Get();
    private readonly IProperty<IDisplayRatio> _dpiAwareAngularDpi = H.Property<IDisplayRatio>(c => c
        .Set(e => new DisplayRatioValue(e.Device.AngularDpi) as IDisplayRatio)
        .On(e => e.Device.AngularDpi).Update()
    );
    // This is the ratio used in system config

    public IDisplayRatio UpdateWpfToPixelRatio()
    {
        switch (Monitor.Layout.DpiAwarenessContext)
        {
            case NativeMethods.DPI_Awareness_Context.Unaware:
                if (RealDpi == null) return null;
                if (DpiAwareAngularDpi == null) return null;
                return new DisplayRatioValue(
                    Math.Round(RealDpi.X / DpiAwareAngularDpi.X * 10) / 10,
                    Math.Round(RealDpi.Y / DpiAwareAngularDpi.Y * 10) / 10);
            //return Math.Round((RealDpiY / DpiAwareAngularDpiY) * 20) / 20;

            case NativeMethods.DPI_Awareness_Context.StrangeValue1:
            case NativeMethods.DPI_Awareness_Context.StrangeValue2:
            case NativeMethods.DPI_Awareness_Context.StrangeValue:
            case NativeMethods.DPI_Awareness_Context.System_Aware:
                if (Monitor.Layout?.PrimarySource == null) return new DisplayRatioValue(1, 1);
                else return new DisplayRatioValue(
                    Monitor.Layout.PrimarySource.EffectiveDpi.X / 96,
                    Monitor.Layout.PrimarySource.EffectiveDpi.Y / 96
                );

            case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware:
                return new DisplayRatioValue(
                    EffectiveDpi.X / 96,
                    EffectiveDpi.Y / 96
                );

            case NativeMethods.DPI_Awareness_Context.Per_Monitor_Aware_V2:
                return new DisplayRatioValue(
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

    public void Load(RegistryKey baseKey)
    {
        using (var key = baseKey.OpenSubKey("GuiLocation"))
        {
            if (key != null)
            {
                var left = key.GetKey("Left", () => GuiLocation.Left);
                var width = key.GetKey("Width", () => GuiLocation.Width);
                var top = key.GetKey("Top", () => GuiLocation.Top);
                var height = key.GetKey("Height", () => GuiLocation.Height);
                _guiLocation.Set(new Rect(new Point(left, top), new Size(width, height)));
            }
        }
    }
    public void Save(RegistryKey baseKey)
    {
        using (var key = baseKey.CreateSubKey("GuiLocation"))
        {
            if(key!=null)
            {
                key.SetKey("Left", GuiLocation.Left);
                key.SetKey("Width", GuiLocation.Width);
                key.SetKey("Top", GuiLocation.Top);
                key.SetKey("Height", GuiLocation.Height);
            }
        }

        using (var key = baseKey.CreateSubKey(Device.IdMonitor))
        {
            if (key != null)
            {
                key.SetKey("PixelX", InPixel.X);
                key.SetKey("PixelY", InPixel.Y);
                key.SetKey("PixelWidth", InPixel.Width);
                key.SetKey("PixelHeight", InPixel.Height);

                key.SetKey("Primary", Primary);
            }
        }
    }

}
