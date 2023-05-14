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
using System.Reactive.Concurrency;
using System.Reactive.Disposables;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using Avalonia;
using DynamicData;

//using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;
using ReactiveUI;


namespace LittleBigMouse.DisplayLayout.Monitors;

[DataContract]
public class PhysicalMonitor : ReactiveObject
{
    public class Design : PhysicalMonitor
    {
        public Design() : base("PNP0000", MonitorsLayout.Design, PhysicalMonitorModel.Design)
        {
        }
    }

    public string IdPhysicalMonitor { get; }

    [JsonIgnore] public IMonitorsLayout Layout { get; }

    /// <summary>
    /// Display sources connected to this physical monitor
    /// </summary>
    public ISourceList<PhysicalSource> Sources { get; } = new SourceList<PhysicalSource>();

    /// <summary>
    /// Actual source currently displayed on this physical monitor
    /// </summary>
    public PhysicalSource ActiveSource
    {
        get => _activeSource;
        set => this.RaiseAndSetIfChanged(ref _activeSource, value);
    }
    PhysicalSource _activeSource;


    //public static Monitor Design => new Monitor(DisplayLayout.Layout.Design, MonitorDevice.Design);

    public PhysicalMonitor(string idPhysicalMonitor, IMonitorsLayout layout, PhysicalMonitorModel model)
    {
        Layout = layout;
        Model = model;
        IdPhysicalMonitor = idPhysicalMonitor;

        _physicalRotated = this.WhenAnyValue(
            e => e.Model.PhysicalSize,
            e => e.Orientation,
            (physicalSize, orientation) => physicalSize.Rotate(orientation)
        ).Log(this, "_physicalRotated").ToProperty(this, e => e.PhysicalRotated);


        //RemainingPhysicalMonitors = Layout.PhysicalMonitors.Items.AsObservableChangeSet().Filter(s => !Equals(s, this)).AsObservableList();

        DepthRatio = new DisplayRatioValue(1.0, 1.0);

        _id = this.WhenAnyValue(
            e => e.ActiveSource.Source.IdMonitor,
            e => e.Orientation,
            (id, o) => $"{id}_{o}"
                ).Log(this, "_id").ToProperty(this, e => e.Id, scheduler: Scheduler.Immediate);

        _depthProjection = this.WhenAnyValue(
            e => e.PhysicalRotated,
            e => e.DepthRatio,
            (physicalRotated, ratio) => physicalRotated.Scale(ratio).Locate()
            ).Log(this, "_inMm").ToProperty(this, e => e.DepthProjection);

        _depthProjectionUnrotated = this.WhenAnyValue(
            e => e.Model.PhysicalSize,
            e => e.DepthRatio,
            (physicalSize, ratio) => physicalSize.Scale(ratio)
            ).Log(this, "_inMmU").ToProperty(this, e => e.DepthProjectionUnrotated);

        _diagonal = this.WhenAnyValue(
            e => e.DepthProjection.Height,
            e => e.DepthProjection.Width,
            (h, w) => Math.Sqrt(w * w + h * h)
            ).Log(this, "_diagonal").ToProperty(this, e => e.Diagonal);


        //_overallBoundsWithoutThisInMm =
        //        Layout.AllMonitors.Items
        //        .AsObservableChangeSet()
        //        .WhenValueChanged(e => e.InMm.Bounds)
        //        .Select(i => GetOverallBoundsWithoutThisInMm())
        //        .Log(this,"_overallBoundsWithoutThisInMm").ToProperty(this, e => e.OverallBoundsWithoutThisInMm);
    }

    public void AddSource(PhysicalSource source) => Sources.Add(source);

    // TODO : Avalonia
    //readonly ITrigger _updateModel = H.Trigger(c => c
    //    .On(e => e.Device.AttachedDisplay)
    //    .Do(s => s.Model.Load(s.Device))
    //);

    /// <summary>
    /// List of other monitors in the layout 
    /// </summary>
    [JsonIgnore] public IObservableList<PhysicalMonitor> RemainingPhysicalMonitors { get; }


    // References properties
    [DataMember] public string Id => _id.Value;
    readonly ObservableAsPropertyHelper<string> _id;


    [DataMember]
    public int Orientation
    {
        get => _orientation;
        set => this.RaiseAndSetIfChanged(ref _orientation, value);
    }
    int _orientation;

    [DataMember]
    public string SerialNumber
    {
        get => _serialNumber;
        set => this.RaiseAndSetIfChanged(ref _serialNumber, value);
    }
    string _serialNumber;

    public bool Placed
    {
        get => _placed;
        set => this.RaiseAndSetIfChanged(ref _placed, value);
    }
    bool _placed;

    // Mm dimensions
    // Natives

    public PhysicalMonitorModel Model { get; }

    [DataMember] public IDisplaySize PhysicalRotated => _physicalRotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _physicalRotated;


    // Mm

    /// <summary>
    /// 
    /// </summary>
    [DataMember]
    public IDisplaySize DepthProjection => _depthProjection.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _depthProjection;

    [DataMember]
    public IDisplaySize DepthProjectionUnrotated => _depthProjectionUnrotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _depthProjectionUnrotated;


    // TODO
    //ITrigger _setUnsaved = H.Trigger(c => c
    //    .On(e => e.InMm.OutsideBounds)
    //    .On(e => e.InMm.Bounds)
    //    .Do(e => e.Layout.Saved = false)
    //);

    /// <summary>
    /// Final ratio to deal with monitor distance
    /// </summary>
    [DataMember]
    public IDisplayRatio DepthRatio { get; }
    public double Diagonal => _diagonal.Value;

    readonly ObservableAsPropertyHelper<double> _diagonal;

    //[DataMember] public Rect OverallBoundsWithoutThisInMm => _overallBoundsWithoutThisInMm.Value;

    //readonly ObservableAsPropertyHelper<Rect> _overallBoundsWithoutThisInMm;

    //Rect GetOverallBoundsWithoutThisInMm()
    //{
    //    return MonitorsLayout.Union(
    //        Layout
    //        .PhysicalMonitorsExcept(this)
    //        .Where(e => e.DepthProjection is not null)
    //        .Select(e => e.DepthProjection.Bounds)
    //    );
    //}







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







    public string InterfaceLogo
    {
        get => _interfaceLogo;
        set => this.RaiseAndSetIfChanged(ref _interfaceLogo, value);
    }
    string _interfaceLogo;


    //    .Set(e => e.InMm.Y)
    //    .On(e => e.Moving)
    //    .On(e => e.InMm.Y)
    //    .When(e => !e.Moving)
    //    .Update()
    //);

    //double LogPixelSx
    //{
    //    get
    //    {
    //        var hdc = CreateDC("DISPLAY", Device.AttachedDisplay.DeviceName, null, 0);
    //        double dpi = GetDeviceCaps(hdc, DeviceCap.LogPixelsX);
    //        DeleteDC(hdc);
    //        return dpi;
    //    }
    //}


    double RightDistance(PhysicalMonitor monitor) => DepthProjection.OutsideBounds.X - monitor.DepthProjection.OutsideBounds.Right;

    double LeftDistance(PhysicalMonitor monitor) => monitor.DepthProjection.OutsideBounds.X - DepthProjection.OutsideBounds.Right;

    double TopDistance(PhysicalMonitor monitor) => monitor.DepthProjection.OutsideBounds.Y - DepthProjection.OutsideBounds.Bottom;

    double BottomDistance(PhysicalMonitor monitor) => DepthProjection.OutsideBounds.Y - monitor.DepthProjection.OutsideBounds.Bottom;

    double RightDistanceToTouch(PhysicalMonitor monitor, bool zero = false)
    {
        var top = TopDistance(monitor);
        if (top > 0 || zero && top == 0) return double.PositiveInfinity;
        var bottom = BottomDistance(monitor);
        if (bottom > 0 || zero && bottom == 0) return double.PositiveInfinity;
        return RightDistance(monitor);
    }

    double RightDistanceToTouch(IEnumerable<PhysicalMonitor> monitors, bool zero = false) 
        => monitors
            .Select(monitor => RightDistanceToTouch(monitor, zero))
            .Prepend(double.PositiveInfinity)
            .Min();

    double RightDistance(IEnumerable<PhysicalMonitor> monitors) 
        => monitors
            .Select(RightDistance)
            .Prepend(double.PositiveInfinity)
            .Min();

    double LeftDistanceToTouch(PhysicalMonitor monitor, bool zero = false)
    {
        var top = TopDistance(monitor);
        if (top > 0 || zero && top == 0) return double.PositiveInfinity;

        var bottom = BottomDistance(monitor);
        if (bottom > 0 || zero && bottom == 0) return double.PositiveInfinity;

        return LeftDistance(monitor);
    }

    double LeftDistanceToTouch(IEnumerable<PhysicalMonitor> monitors, bool zero = false) 
        => monitors
            .Select(monitor => LeftDistanceToTouch(monitor, zero))
            .Prepend(double.PositiveInfinity)
            .Min();

    double LeftDistance(IEnumerable<PhysicalMonitor> monitors) 
        => monitors
            .Select(LeftDistance)
            .Prepend(double.PositiveInfinity)
            .Min();

    double TopDistanceToTouch(PhysicalMonitor monitor, bool zero = false)
    {
        var left = LeftDistance(monitor);
        if (left > 0 || zero && left == 0) return double.PositiveInfinity;

        var right = RightDistance(monitor);
        if (right > 0 || zero && right == 0) return double.PositiveInfinity;

        return TopDistance(monitor);
    }

    double TopDistanceToTouch(IEnumerable<PhysicalMonitor> monitors, bool zero = false) 
        => monitors.Select(monitor => TopDistanceToTouch(monitor, zero)).Prepend(double.PositiveInfinity).Min();

    double TopDistance(IEnumerable<PhysicalMonitor> monitors) 
        => monitors.Select(TopDistance).Prepend(double.PositiveInfinity).Min();

    double BottomDistanceToTouch(PhysicalMonitor monitor, bool zero = false)
    {
        var left = LeftDistance(monitor);
        if (left > 0 || zero && Math.Abs(left) < double.Epsilon) return double.PositiveInfinity;

        var right = RightDistance(monitor);
        if (right > 0 || zero && Math.Abs(right) < double.Epsilon) return double.PositiveInfinity;

        return BottomDistance(monitor);
    }

    double BottomDistanceToTouch(IEnumerable<PhysicalMonitor> monitors, bool zero = false) 
        => monitors
            .Select(monitor => BottomDistanceToTouch(monitor, zero))
            .Prepend(double.PositiveInfinity)
            .Min();

    double BottomDistance(IEnumerable<PhysicalMonitor> monitors) 
        => monitors
            .Select(BottomDistance)
            .Prepend(double.PositiveInfinity)
            .Min();

    public double HorizontalDistance(PhysicalMonitor monitor)
    {
        double right = RightDistance(monitor);
        if (right >= 0) return right;

        double left = LeftDistance(monitor);
        if (left >= 0) return left;

        return Math.Max(right, left);
    }

    public double HorizontalDistance(IEnumerable<PhysicalMonitor> monitors)
    {
        var right = RightDistanceToTouch(monitors);
        if (right >= 0) return right;

        var left = LeftDistanceToTouch(monitors);
        if (left >= 0) return left;

        return Math.Max(right, left);
    }

    public double VerticalDistance(IEnumerable<PhysicalMonitor> monitors)
    {
        var top = TopDistanceToTouch(monitors);
        if (top >= 0) return top;

        var bottom = BottomDistanceToTouch(monitors);
        if (bottom >= 0) return bottom;

        return Math.Max(top, bottom);
    }

    public double VerticalDistance(PhysicalMonitor monitor)
    {
        var top = TopDistance(monitor);
        if (top >= 0) return top;

        var bottom = BottomDistance(monitor);
        if (bottom >= 0) return bottom;

        return Math.Max(top, bottom);
    }

    public double Distance(PhysicalMonitor monitor)
    {
        var v = new Vector(HorizontalDistance(monitor), VerticalDistance(monitor));

        if (v.X >= 0 && v.Y >= 0) return v.Length;

        if (v.X >= 0) return v.X;
        if (v.Y >= 0) return v.Y;

        return Math.Max(v.X, v.Y);
    }

    public double Distance(IEnumerable<PhysicalMonitor> monitors)
    {
        var v = new Vector(HorizontalDistance(monitors), VerticalDistance(monitors));

        if (v is { X: >= 0, Y: >= 0 }) return v.Length;

        if (v.X >= 0) return v.X;
        if (v.Y >= 0) return v.Y;

        return Math.Max(v.X, v.Y);
    }

    public bool Expand(PhysicalMonitor s)
    {
        var moveLeft = -LeftDistance(s); if (moveLeft <= 0) return false;

        var moveRight = -RightDistance(s); if (moveRight <= 0) return false;

        var moveUp = -TopDistance(s); if (moveUp <= 0) return false;

        var moveDown = -BottomDistance(s); if (moveDown <= 0) return false;

        if (moveLeft <= moveRight && moveLeft <= moveUp && moveLeft <= moveDown)
        {
            s.DepthProjection.X -= moveLeft;
        }
        else if (moveRight <= moveLeft && moveRight <= moveUp && moveRight <= moveDown)
        {
            s.DepthProjection.Y += moveRight;
        }
        else if (moveUp <= moveRight && moveUp <= moveLeft && moveUp <= moveDown)
        {
            s.DepthProjection.Y -= moveUp;
        }
        else
        {
            s.DepthProjection.Y += moveDown;
        }

        return true;
    }

    public bool PhysicalOverlapWith(PhysicalMonitor monitor)
    {
        if (DepthProjection.X >= monitor.DepthProjection.Bounds.Right) return false;
        if (monitor.DepthProjection.X >= DepthProjection.Bounds.Right) return false;
        if (DepthProjection.Y >= monitor.DepthProjection.Bounds.Bottom) return false;
        if (monitor.DepthProjection.Y >= DepthProjection.Bounds.Bottom) return false;

        return true;
    }

    public bool PhysicalTouch(PhysicalMonitor monitor)
    {
        if (PhysicalOverlapWith(monitor)) return false;
        if (DepthProjection.X > monitor.DepthProjection.Bounds.Right) return false;
        if (monitor.DepthProjection.X > DepthProjection.Bounds.Right) return false;
        if (DepthProjection.Y > monitor.DepthProjection.Bounds.Bottom) return false;
        if (monitor.DepthProjection.Y > DepthProjection.Bounds.Bottom) return false;

        return true;
    }

    public double MoveLeftToTouch(PhysicalMonitor monitor)
    {
        if (DepthProjection.Y >= monitor.DepthProjection.Bounds.Bottom) return -1;
        if (monitor.DepthProjection.Y >= DepthProjection.Bounds.Bottom) return -1;
        return DepthProjection.X - monitor.DepthProjection.Bounds.Right;
    }

    public double MoveRightToTouch(PhysicalMonitor monitor)
    {
        if (DepthProjection.Y >= monitor.DepthProjection.Bounds.Bottom) return -1;
        if (monitor.DepthProjection.Y >= DepthProjection.Bounds.Bottom) return -1;
        return monitor.DepthProjection.X - DepthProjection.Bounds.Right;
    }

    public double MoveUpToTouch(PhysicalMonitor monitor)
    {
        if (DepthProjection.X > monitor.DepthProjection.Bounds.Right) return -1;
        if (monitor.DepthProjection.X > DepthProjection.Bounds.Right) return -1;
        return DepthProjection.Y - monitor.DepthProjection.Bounds.Bottom;
    }

    public double MoveDownToTouch(PhysicalMonitor monitor)
    {
        if (DepthProjection.X > monitor.DepthProjection.Bounds.Right) return -1;
        if (monitor.DepthProjection.X > DepthProjection.Bounds.Right) return -1;
        return monitor.DepthProjection.Y - DepthProjection.Bounds.Bottom;
    }


    public void PlaceAuto(IEnumerable<PhysicalMonitor> monitors, MonitorsLayout layout)
    {
        var left = LeftDistanceToTouch(monitors, true);
        var right = RightDistanceToTouch(monitors, true);
        var top = TopDistanceToTouch(monitors, true);
        var bottom = BottomDistanceToTouch(monitors, true);

        if (!layout.AllowDiscontinuity && double.IsPositiveInfinity(left) && double.IsPositiveInfinity(top) && double.IsPositiveInfinity(right) && double.IsPositiveInfinity(bottom))
        {
            top = TopDistance(monitors);
            right = RightDistance(monitors);
            bottom = BottomDistance(monitors);
            left = LeftDistance(monitors);

            if (left > 0)
            {
                if (top > 0)
                {
                    DepthProjection.X += LeftDistance(monitors);
                    DepthProjection.Y += TopDistance(monitors);
                }
                if (bottom > 0)
                {
                    DepthProjection.X += LeftDistance(monitors);
                    DepthProjection.Y -= BottomDistance(monitors);
                }
            }
            if (right > 0)
            {
                if (top > 0)
                {
                    DepthProjection.X -= RightDistance(monitors);
                    DepthProjection.Y += TopDistance(monitors);
                }
                if (bottom > 0)
                {
                    DepthProjection.X -= RightDistance(monitors);
                    DepthProjection.Y -= BottomDistance(monitors);
                }
            }

            left = LeftDistanceToTouch(monitors, false);
            right = RightDistanceToTouch(monitors, false);
            top = TopDistanceToTouch(monitors, false);
            bottom = BottomDistanceToTouch(monitors, false);
        }

        if (!layout.AllowDiscontinuity)
        {

            if (top > 0 && left > 0)
            {
                if (left < top) DepthProjection.X += left;
                else DepthProjection.Y += top;
                return;
            }

            if (top > 0 && right > 0)
            {
                if (right < top) DepthProjection.X -= right;
                else DepthProjection.Y += top;
                return;
            }

            if (bottom > 0 && right > 0)
            {
                if (right < bottom) DepthProjection.X -= right;
                else DepthProjection.Y -= bottom;
                return;
            }

            if (bottom > 0 && left > 0)
            {
                if (left < bottom) DepthProjection.X += left;
                else DepthProjection.Y -= bottom;
                return;
            }

            if (top < 0 && bottom < 0)
            {
                if (left >= 0)
                {
                    DepthProjection.X += left;
                    return;
                }
                if (right >= 0)
                {
                    DepthProjection.X -= right;
                    return;
                }
            }

            if (left < 0 && right < 0)
            {
                //if (top >= 0)
                if (top > 0)
                {
                    DepthProjection.Y += top;
                    return;
                }
                if (bottom >= 0)
                {
                    DepthProjection.Y -= bottom;
                    return;
                }
            }
        }

        if (!layout.AllowOverlaps && left < 0 && right < 0 && top < 0 && bottom < 0)
        {
            if (left > right && left > top && left > bottom)
            {
                DepthProjection.X += left;
            }
            else if (right > top && right > bottom)
            {
                DepthProjection.X -= right;
            }
            else if (top > bottom)
            {
                DepthProjection.Y += top;
            }
            else DepthProjection.Y -= bottom;
        }
    }

    public override string ToString() => $"{this.Id}";
}

