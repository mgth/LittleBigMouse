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
using System.Collections.Generic;
using System.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using Avalonia;
using DynamicData;
using HLab.Sys.Windows.API;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;
using ReactiveUI;
using Rect = Avalonia.Rect;

namespace LittleBigMouse.DisplayLayout;

[DataContract]
public class Monitor : ReactiveObject
{
    [JsonIgnore] public Layout Layout { get; }

    public ISourceList<MonitorSource> Sources { get; } = new SourceList<MonitorSource>();

    public MonitorSource ActiveSource
    {
        get => _activeSource;
        set => this.RaiseAndSetIfChanged(ref _activeSource,value);
    }
    MonitorSource _activeSource;

    public MonitorDevice Device { get; }

    public Monitor(Layout layout, MonitorDevice device)
    {
        Layout = layout;
        Device = device;

        Model = Layout.GetScreenModel(device.PnpCode, device);

        var source = new MonitorSource(this, device);

        Sources.Add(source);
        ActiveSource = source;

        OtherScreens = Layout.AllMonitors.Connect().Filter(s => !Equals(s, this)).AsObservableList();

        this.WhenAnyValue(
            e => e.Device.IdMonitor,
            e => e.Orientation,
            (id, o) => $"{id}_{o}"
        ).ToProperty(this,e => e.Id, out _id);

        this.WhenAnyValue(
            e => e.Device.AttachedDisplay.CurrentMode.DisplayOrientation
        ).ToProperty(this,e => e.Orientation, out _orientation);

        this.WhenAnyValue(
            e => e.Model.PhysicalSize,
            e => e.Orientation,
            (physicalSize, orientation) => physicalSize.Rotate(orientation)
        ).ToProperty(this,e => e.PhysicalRotated, out _physicalRotated);

        this.WhenAnyValue(
            e => e.PhysicalRotated,
            e => e.PhysicalRatio,
            (physicalRotated, ratio) => physicalRotated.Scale(ratio).Locate()
        ).ToProperty(this,e => e.InMm, out _inMm);

        this.WhenAnyValue(
            e => e.Model.PhysicalSize,
            e => e.PhysicalRatio,
            (physicalSize, ratio) => physicalSize.Scale(ratio)
        ).ToProperty(this,e => e.InMmU, out _inMmU);

        this.WhenAnyValue(
            e => e.InMm.X
        ).ToProperty(this,e => e.InMmX, out _inMmX);

        this.WhenAnyValue(
            e => e.InMm.Y
        ).ToProperty(this,e => e.InMmY, out _inMmY);

        PhysicalRatio = new DisplayRatioValue(1.0, 1.0);

        this.WhenAnyValue(
                e => e.InMm.Height,
                e => e.InMm.Width,
                (h, w) => Math.Sqrt(w * w + h * h))
            .ToProperty(this, e=> e.Diagonal, out _diagonal);
    }

    public void AddSource(MonitorSource source)
        => Sources.Add(source);
    // TODO :
    //readonly ITrigger _updateModel = H.Trigger(c => c
    //    .On(e => e.Device.AttachedDisplay)
    //    .Do(s => s.Model.Load(s.Device))
    //);

    [JsonIgnore] public IObservableList<Monitor> OtherScreens { get; }


    // References properties
    [DataMember] public string Id => _id.Get();
    readonly ObservableAsPropertyHelper<string> _id;

    [DataMember] public int Orientation => _orientation.Value;
    readonly ObservableAsPropertyHelper<int> _orientation;

    public bool Selected
    {
        get => _selected;
        set
        {
            using (DelayChangeNotifications())
            {
                this.RaiseAndSetIfChanged(ref _selected, value);
                if (!value) return;
                foreach (var screen in Layout.AllBut(this)) screen.Selected = false;
            }
        }
    }

    bool _selected;

    public bool Placed
    {
        get => _placed;
        set => this.RaiseAndSetIfChanged(ref _placed, value);
    }
    bool _placed;

    // Mm dimensions
    // Natives

    public MonitorModel Model { get; }

    [DataMember] public IDisplaySize PhysicalRotated => _physicalRotated.Get();

    readonly ObservableAsPropertyHelper<IDisplaySize> _physicalRotated;


    // Mm

    [DataMember]
    public IDisplaySize InMm => _inMm.Get();

    readonly ObservableAsPropertyHelper<IDisplaySize> _inMm;

    [DataMember]
    public IDisplaySize InMmU => _inMmU.Get();

    readonly ObservableAsPropertyHelper<IDisplaySize> _inMmU;

    public double InMmX
    {
        get => _inMmX.Get();
        set
        {
            if (Sources.Items.Any(s => s.Primary))
            {
                foreach (var screen in Layout.AllBut(this))
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

    readonly ObservableAsPropertyHelper<double> _inMmX;

    public double InMmY
    {
        get => _inMmY.Get();
        set
        {
            if (Sources.Items.Any(s => s.Primary))
            {
                foreach (var screen in Layout.AllBut(this))
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

    readonly ObservableAsPropertyHelper<double> _inMmY;
    
    // TODO
    //ITrigger _setUnsaved = H.Trigger(c => c
    //    .On(e => e.InMm.OutsideBounds)
    //    .On(e => e.InMm.Bounds)
    //    .Do(e => e.Layout.Saved = false)
    //);

    /// <summary>
    /// Final ratio to deal with screen distance
    /// </summary>
    [DataMember]
    public IDisplayRatio PhysicalRatio { get; }

    public double Diagonal => _diagonal.Get();

    readonly ObservableAsPropertyHelper<double> _diagonal;

    [DataMember] public Rect OverallBoundsWithoutThisInMm => _overallBoundsWithoutThisInMm.Get();

    readonly ObservableAsPropertyHelper<Rect> _overallBoundsWithoutThisInMm;

    // TODO : .On(e => e.Layout.AllMonitors.Item().InMm.Bounds).Update()
    static Rect GetOverallBoundsWithoutThisInMm(Layout layout, Monitor monitor)
    {
        return Layout.Union(layout
            .AllBut(monitor)
            .Where(e => e.InMm is not null)
            .Select(e => e.InMm.Bounds)
        );
    }


    public static RegistryKey OpenRegKey(string LayoutId, string monitorId, bool create = false) => OpenRegKey(Layout.OpenRegKey(LayoutId,create),monitorId,create);

    public static RegistryKey OpenRegKey(RegistryKey baseKey, string monitorId, bool create = false)
    {
        return create?
            baseKey.CreateSubKey(monitorId):
            baseKey.OpenSubKey(monitorId);
    }

    public RegistryKey OpenRegKey(bool create = false) => OpenRegKey(Layout.OpenRegKey(create),create);// OpenRegKey(Layout.Id, Device.IdPhysicalMonitor, create);
    public RegistryKey OpenRegKey(RegistryKey baseKey, bool create = false) => OpenRegKey(baseKey, Device.IdPhysicalMonitor, create);

    public void Load(RegistryKey baseKey)
    {
        using var key = OpenRegKey(baseKey);

        if (key != null)
        {
            InMm.X = key.GetKey("XLocationInMm", () => InMm.X, () => Placed = true);
            InMm.Y = key.GetKey("YLocationInMm", () => InMm.Y, () => Placed = true);
            PhysicalRatio.X = key.GetKey("PhysicalRatioX", () => PhysicalRatio.X);
            PhysicalRatio.Y = key.GetKey("PhysicalRatioY", () => PhysicalRatio.Y);
        }

        var active = key.GetKey("ActiveSource", () => "");
        foreach(var source in Sources.Items)
        {
            source.Load(key);
            if(source.Device.IdMonitor == active || ActiveSource==null)
                ActiveSource = source;
        }

    }

    public void Save(RegistryKey baseKey)
    {
        Model.Save(baseKey);

        using (var key = Device.OpenMonitorRegKey(true))
        {
            key?.SetKey("DeviceId", Device.DeviceId);
        }

        using (var key = OpenRegKey(baseKey,true))
        {
            if (key != null)
            {
                key.SetKey("XLocationInMm", InMm.X);
                key.SetKey("YLocationInMm", InMm.Y);
                key.SetKey("PhysicalRatioX", PhysicalRatio.X);
                key.SetKey("PhysicalRatioY", PhysicalRatio.Y);

                foreach(var source in Sources.Items)
                {
                    source.Save(key);
                }

                key.SetKey("ActiveSource", ActiveSource.Device.IdMonitor);
                key.SetKey("Orientation", Orientation);
            }
        }
    }

    //[TriggerOn(nameof(Monitor), "WorkArea")]
    //public AbsoluteRectangle AbsoluteWorkingArea => this.Get(() => new AbsoluteRectangle(
    //    new PixelPoint(Config, this, Monitor.WorkArea.X, Monitor.WorkArea.Y),
    //    new PixelPoint(Config, this, Monitor.WorkArea.Right, Monitor.WorkArea.Bottom)
    //));

    static readonly List<string> SideNames = new List<string> { "Left", "Top", "Right", "Bottom" };
    static readonly List<string> DimNames = new List<string> { "Width", "Height" };

    //private double LoadRotatedValue(List<string> names, string name, string side, Func<double> def,
    //    bool fromConfig = false)
    //{
    //    int n = names.Count;
    //    int pos = (names.IndexOf(side) + Orientation) % n;
    //    using (RegistryKey key = fromConfig ? OpenLayoutRegKey() : Device.OpenMonitorRegKey())
    //    {
    //        return key.GetKey(name.Replace("%", names[pos]), def);
    //    }
    //}


    //private void SaveRotatedValue(List<string> names, string name, string side, double value,
    //    bool fromConfig = false)
    //{
    //    int n = names.Count;
    //    int pos = (names.IndexOf(side) + n - Orientation) % n;
    //    using (RegistryKey key = fromConfig ? OpenLayoutRegKey(true) : Device.OpenMonitorRegKey(true))
    //    {
    //        key.SetKey(name.Replace("%", names[pos]), value);
    //    }
    //}





    //        private NativeMethods.Process_DPI_Awareness DpiAwareness => this.Get(() =>
    //        {
    ////            Process p = Process.GetCurrentProcess();

    //            NativeMethods.Process_DPI_Awareness aw = NativeMethods.Process_DPI_Awareness.Per_Monitor_DPI_Aware;

    //            NativeMethods.GetProcessDpiAwareness(/*p.Handle*/IntPtr.Zero, out aw);

    //            return aw;
    //        });





    public bool Moving
    {
        get => _moving;
        set
        {
            var x = XMoving;
            var y = YMoving;

            var oldValue = value;
            using (DelayChangeNotifications())
            {
                if (this.RaiseAndSetIfChanged(ref _moving, value) == oldValue && !value) return;
                InMmX = x;
                InMmY = y;
            }
        }
    }
    bool _moving;

    public double XMoving
    {
        get => _xMoving;
        set
        {
            if (Moving) this.RaiseAndSetIfChanged(ref _xMoving, value);
            else InMmX = value;
        }
    }

    double _xMoving;
    // TODO :
    //    .Set(e => e.InMm.X)
    //    .On(e => e.Moving)
    //    .On(e => e.InMm.X)
    //    .When(e => !e.Moving)
    //    .Update()
    //);

    public double YMoving
    {
        get => _yMoving;
        set
        {
            if (Moving) this.RaiseAndSetIfChanged(ref _yMoving, value);
            else InMmY = value;
        }
    }
    double _yMoving;

    //    .Set(e => e.InMm.Y)
    //    .On(e => e.Moving)
    //    .On(e => e.InMm.Y)
    //    .When(e => !e.Moving)
    //    .Update()
    //);

    double LogPixelSx
    {
        get
        {
            var hdc = Gdi32.CreateDC("DISPLAY", Device.AttachedDisplay.DeviceName, null, IntPtr.Zero);
            double dpi = Gdi32.GetDeviceCaps(hdc, Gdi32.DeviceCap.LOGPIXELSX);
            Gdi32.DeleteDC(hdc);
            return dpi;
        }
    }


    double RightDistance(Monitor screen) => InMm.OutsideBounds.X - screen.InMm.OutsideBounds.Right;
    double LeftDistance(Monitor screen) => screen.InMm.OutsideBounds.X - InMm.OutsideBounds.Right;
    double TopDistance(Monitor screen) => screen.InMm.OutsideBounds.Y - InMm.OutsideBounds.Bottom;
    double BottomDistance(Monitor screen) => InMm.OutsideBounds.Y - screen.InMm.OutsideBounds.Bottom;

    double RightDistanceToTouch(Monitor screen, bool zero = false)
    {
        double top = TopDistance(screen);
        if (top > 0 || zero && top == 0) return double.PositiveInfinity;
        double bottom = BottomDistance(screen);
        if (bottom > 0 || zero && bottom == 0) return double.PositiveInfinity;
        return RightDistance(screen);
    }

    double RightDistanceToTouch(IEnumerable<Monitor> screens, bool zero = false)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(RightDistanceToTouch(screen, zero), dist);
        }
        return dist;
    }

    double RightDistance(IEnumerable<Monitor> screens)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(RightDistance(screen), dist);
        }
        return dist;
    }

    double LeftDistanceToTouch(Monitor screen, bool zero = false)
    {
        double top = TopDistance(screen);
        if (top > 0 || zero && top == 0) return double.PositiveInfinity;
        double bottom = BottomDistance(screen);
        if (bottom > 0 || zero && bottom == 0) return double.PositiveInfinity;
        return LeftDistance(screen);
    }
    double LeftDistanceToTouch(IEnumerable<Monitor> screens, bool zero = false)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(LeftDistanceToTouch(screen, zero), dist);
        }
        return dist;
    }

    double LeftDistance(IEnumerable<Monitor> screens)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(LeftDistance(screen), dist);
        }
        return dist;
    }


    double TopDistanceToTouch(Monitor screen, bool zero = false)
    {
        double left = LeftDistance(screen);
        if (left > 0 || zero && left == 0) return double.PositiveInfinity;
        double right = RightDistance(screen);
        if (right > 0 || zero && right == 0) return double.PositiveInfinity;
        return TopDistance(screen);
    }

    double TopDistanceToTouch(IEnumerable<Monitor> screens, bool zero = false)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(TopDistanceToTouch(screen, zero), dist);
        }
        return dist;
    }

    double TopDistance(IEnumerable<Monitor> screens)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(TopDistance(screen), dist);
        }
        return dist;
    }

    double BottomDistanceToTouch(Monitor screen, bool zero = false)
    {
        var left = LeftDistance(screen);
        if (left > 0 || zero && Math.Abs(left) < double.Epsilon) return double.PositiveInfinity;
        var right = RightDistance(screen);
        if (right > 0 || zero && Math.Abs(right) < double.Epsilon) return double.PositiveInfinity;
        return BottomDistance(screen);
    }

    double BottomDistanceToTouch(IEnumerable<Monitor> screens, bool zero = false)
    {
        var dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(BottomDistanceToTouch(screen, zero), dist);
        }
        return dist;
    }

    double BottomDistance(IEnumerable<Monitor> screens)
    {
        double dist = double.PositiveInfinity;
        foreach (Monitor screen in screens)
        {
            dist = Math.Min(BottomDistance(screen), dist);
        }
        return dist;
    }


    public double HorizontalDistance(Monitor screen)
    {
        double right = RightDistance(screen);
        if (right >= 0) return right;

        double left = LeftDistance(screen);
        if (left >= 0) return left;

        return Math.Max(right, left);
    }

    public double HorizontalDistance(IEnumerable<Monitor> screens)
    {
        double right = RightDistanceToTouch(screens);
        if (right >= 0) return right;

        double left = LeftDistanceToTouch(screens);
        if (left >= 0) return left;

        return Math.Max(right, left);
    }

    public double VerticalDistance(IEnumerable<Monitor> screens)
    {
        double top = TopDistanceToTouch(screens);
        if (top >= 0) return top;

        double bottom = BottomDistanceToTouch(screens);
        if (bottom >= 0) return bottom;

        return Math.Max(top, bottom);
    }

    public double VerticalDistance(Monitor screen)
    {
        double top = TopDistance(screen);
        if (top >= 0) return top;

        double bottom = BottomDistance(screen);
        if (bottom >= 0) return bottom;

        return Math.Max(top, bottom);
    }

    public double Distance(Monitor screen)
    {
        var v = new Vector(HorizontalDistance(screen), VerticalDistance(screen));

        if (v.X >= 0 && v.Y >= 0) return v.Length;

        if (v.X >= 0) return v.X;
        if (v.Y >= 0) return v.Y;

        return Math.Max(v.X, v.Y);
    }

    public double Distance(IEnumerable<Monitor> screens)
    {
        Vector v = new Vector(HorizontalDistance(screens), VerticalDistance(screens));

        if (v.X >= 0 && v.Y >= 0) return v.Length;

        if (v.X >= 0) return v.X;
        if (v.Y >= 0) return v.Y;

        return Math.Max(v.X, v.Y);
    }

    public bool Expand(Monitor s)
    {
        double moveLeft = -LeftDistance(s); if (moveLeft <= 0) return false;

        double moveRight = -RightDistance(s); if (moveRight <= 0) return false;

        double moveUp = -TopDistance(s); if (moveUp <= 0) return false;

        double moveDown = -BottomDistance(s); if (moveDown <= 0) return false;

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

    public bool PhysicalOverlapWith(Monitor screen)
    {
        if (InMm.X >= screen.InMm.Bounds.Right) return false;
        if (screen.InMm.X >= InMm.Bounds.Right) return false;
        if (InMm.Y >= screen.InMm.Bounds.Bottom) return false;
        if (screen.InMm.Y >= InMm.Bounds.Bottom) return false;

        return true;
    }

    public bool PhysicalTouch(Monitor screen)
    {
        if (PhysicalOverlapWith(screen)) return false;
        if (InMm.X > screen.InMm.Bounds.Right) return false;
        if (screen.InMm.X > InMm.Bounds.Right) return false;
        if (InMm.Y > screen.InMm.Bounds.Bottom) return false;
        if (screen.InMm.Y > InMm.Bounds.Bottom) return false;

        return true;
    }

    public double MoveLeftToTouch(Monitor screen)
    {
        if (InMm.Y >= screen.InMm.Bounds.Bottom) return -1;
        if (screen.InMm.Y >= InMm.Bounds.Bottom) return -1;
        return InMm.X - screen.InMm.Bounds.Right;
    }

    public double MoveRightToTouch(Monitor screen)
    {
        if (InMm.Y >= screen.InMm.Bounds.Bottom) return -1;
        if (screen.InMm.Y >= InMm.Bounds.Bottom) return -1;
        return screen.InMm.X - InMm.Bounds.Right;
    }

    public double MoveUpToTouch(Monitor screen)
    {
        if (InMm.X > screen.InMm.Bounds.Right) return -1;
        if (screen.InMm.X > InMm.Bounds.Right) return -1;
        return InMm.Y - screen.InMm.Bounds.Bottom;
    }

    public double MoveDownToTouch(Monitor screen)
    {
        if (InMm.X > screen.InMm.Bounds.Right) return -1;
        if (screen.InMm.X > InMm.Bounds.Right) return -1;
        return screen.InMm.Y - InMm.Bounds.Bottom;
    }


    public void PlaceAuto(IEnumerable<Monitor> screens)
    {
        double left = LeftDistanceToTouch(screens, true);
        double right = RightDistanceToTouch(screens, true);
        double top = TopDistanceToTouch(screens, true);
        double bottom = BottomDistanceToTouch(screens, true);

        if (!Layout.AllowDiscontinuity && double.IsPositiveInfinity(left) && double.IsPositiveInfinity(top) && double.IsPositiveInfinity(right) && double.IsPositiveInfinity(bottom))
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

            left = LeftDistanceToTouch(screens, false);
            right = RightDistanceToTouch(screens, false);
            top = TopDistanceToTouch(screens, false);
            bottom = BottomDistanceToTouch(screens, false);
        }

        if (!Layout.AllowDiscontinuity)
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

        if (!Layout.AllowOverlaps && left < 0 && right < 0 && top < 0 && bottom < 0)
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

