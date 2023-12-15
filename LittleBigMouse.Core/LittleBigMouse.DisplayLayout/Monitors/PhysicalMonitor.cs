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
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using DynamicData;
using HLab.Base.Avalonia;
using HLab.Base.Avalonia.Extensions;

//using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using ReactiveUI;


namespace LittleBigMouse.DisplayLayout.Monitors;

[DataContract]
public class PhysicalMonitor : ReactiveModel
{
    public class Design : PhysicalMonitor
    {
        public Design() : base("PNP0000", MonitorsLayout.MonitorsLayoutDesign, PhysicalMonitorModel.Design)
        {
            if(!Avalonia.Controls.Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
        }
    }

    [DataMember] public string Id { get; }

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
        set => SetUnsavedValue(ref _activeSource, value);
    }
    PhysicalSource _activeSource;

    [DataMember] 
    public string DeviceId 
    {
        get => _deviceId;
        set => SetUnsavedValue(ref _deviceId, value);
    }
    string _deviceId;

    public PhysicalMonitor(string id, IMonitorsLayout layout, PhysicalMonitorModel model)
    {
        Id = id;
        Layout = layout;
        Model = model;

        Sources.Connect()
            .AutoRefresh(e => e.Saved)
            .ToCollection()
            .Do(ParseDisplaySources)
            .Subscribe().DisposeWith(this);

        _physicalRotated = this.WhenAnyValue(
            e => e.Model.PhysicalSize,
            e => e.ActiveSource.Source.Orientation,
            (physicalSize, orientation) => physicalSize.Rotate(orientation)
        ).Log(this, "_physicalRotated").ToProperty(this, e => e.PhysicalRotated);

        //RemainingPhysicalMonitors = Layout.PhysicalMonitors.Items.AsObservableChangeSet().Filter(s => !Equals(s, this)).AsObservableList();

        DepthRatio = new DisplayRatioValue(1.0, 1.0);

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

        this.WhenAnyValue(e => e.Model.Saved)
            .Subscribe(e =>
            {
                if (e) return;
                Saved = false;
            });

        this.WhenAnyValue(e => e.DepthProjection.Saved)
            .Subscribe(e =>
            {
                if (e) return;
                Saved = false;
            });

        this.WhenAnyValue(e => e.DepthRatio.Saved)
            .Subscribe(e =>
            {
                if (e) return;
                Saved = false;
            });
    }

    void ParseDisplaySources(IReadOnlyCollection<PhysicalSource> obj)
    {
        if (obj.Any(s => !s.Saved))
        {
            Saved = false;
        }
    }

    // References properties

    /// <summary>
    /// Monitor orientation (0=0°, 1=90°, 2=180°, 3=270°)
    /// </summary>
    //[DataMember]
    //public int Orientation
    //{
    //    get => _orientation;
    //    set => this.SetUnsavedValue(ref _orientation, value);
    //}
    //int _orientation;

    /// <summary>
    /// Show each source as a separate monitor
    /// </summary>
    [DataMember]
    public bool SplitSources
    {
        get => _splitSources;
        set => this.SetUnsavedValue(ref _splitSources, value);
    }
    bool _splitSources;

    /// <summary>
    /// Serial number from EDID
    /// </summary>
    [DataMember]
    public string SerialNumber
    {
        get => _serialNumber;
        set => this.SetUnsavedValue(ref _serialNumber, value);
    }
    string _serialNumber;

    /// <summary>
    /// True when placement has been set by user or by automatic placement
    /// </summary>
    public bool Placed
    {
        get => _placed;
        set => this.RaiseAndSetIfChanged(ref _placed, value);
    }
    bool _placed;

    /// <summary>
    /// Monitor model
    /// </summary>
    public PhysicalMonitorModel Model { get; }

    /// <summary>
    /// Dimensions with rotation applied
    /// </summary>
    [DataMember] public IDisplaySize PhysicalRotated => _physicalRotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _physicalRotated;


    // Mm

    /// <summary>
    /// Dimensions with depth ratio applied to deal with monitor distance
    /// </summary>
    [DataMember]
    public IDisplaySize DepthProjection => _depthProjection.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _depthProjection;

    /// <summary>
    /// Dimensions with depth ratio applied but without rotation
    /// </summary>
    [DataMember]
    public IDisplaySize DepthProjectionUnrotated => _depthProjectionUnrotated.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _depthProjectionUnrotated;

    /// <summary>
    /// Final ratio to deal with monitor distance
    /// </summary>
    [DataMember]
    public IDisplayRatio DepthRatio { get; }

    /// <summary>
    /// Diagonal
    /// </summary>
    public double Diagonal => _diagonal.Value;
    readonly ObservableAsPropertyHelper<double> _diagonal;

    /// <summary>
    /// Adjust monitor position to touch another monitor.
    /// </summary>
    /// <param name="monitors">List of other monitors to take into account</param>
    /// <param name="allowDiscontinuity">Allow monitors to not touch each other</param>
    /// <param name="allowOverlaps">Allow monitors to overlap</param>
    public void PlaceAuto(IEnumerable<PhysicalMonitor> monitors, bool allowDiscontinuity=false, bool allowOverlaps=false)
    {
        //Calculation of the distance to the closest monitor
        var distance = this.DepthProjection.OutsideBounds.DistanceToTouch(monitors.Select(m => m.DepthProjection.OutsideBounds), true);

        //If the monitor cannot touch by single translation
        if (!allowDiscontinuity && distance.IsPositiveInfinity())
        {
            var left = monitors.Min(m => m.DepthProjection.OutsideBounds.Left);
            var top = monitors.Min(m => m.DepthProjection.OutsideBounds.Top);
            var right = monitors.Max(m => m.DepthProjection.OutsideBounds.Right);
            var bottom = monitors.Max(m => m.DepthProjection.OutsideBounds.Bottom);

            var toLeft = left - DepthProjection.OutsideBounds.Right;
            var toTop = top - DepthProjection.OutsideBounds.Bottom;
            var toRight = DepthProjection.OutsideBounds.Left - right;
            var toBottom = DepthProjection.OutsideBounds.Top - bottom;

            if (toLeft >= 0)
            {    
                if (toTop >= 0)
                {
                    if(toLeft < toTop) DepthProjection.X = left - DepthProjection.OutsideWidth / 2;
                    else DepthProjection.Y = top - DepthProjection.OutsideHeight / 2;
                }
                else if (toBottom >= 0)
                {
                    if(toLeft < toBottom) DepthProjection.X = left - DepthProjection.OutsideWidth / 2;
                    else DepthProjection.Y = bottom - DepthProjection.OutsideHeight / 2;
                }
            }
            else if (toRight >= 0)
            {
                if (toTop >= 0)
                {
                    if(toRight < toTop) DepthProjection.X = right - DepthProjection.OutsideWidth / 2;
                    else DepthProjection.Y = top - DepthProjection.OutsideHeight / 2;
                }
                else if (toBottom >= 0)
                {
                    if(toRight < toBottom) DepthProjection.X = right - DepthProjection.OutsideWidth / 2;
                    else DepthProjection.Y = bottom - DepthProjection.OutsideHeight / 2;
                }
            }

            distance = this.DepthProjection.OutsideBounds.DistanceToTouch(monitors.Select(m => m.DepthProjection.OutsideBounds), true);    
        }

        var min = distance.MinPositive();
        // Move to touch
        if (!allowDiscontinuity && min>0 && !double.IsInfinity(min))
        {

            if (distance.Left > 0 && distance.Left <= min)
            {
                DepthProjection.X -= distance.Left;
            }
            else if (distance.Top > 0 && distance.Top <= min)
            {
                DepthProjection.Y -= distance.Top;
            }
            else if (distance.Right > 0 && distance.Right <= min)
            {
                DepthProjection.X += distance.Right;
            }
            else if (distance.Bottom > 0 && distance.Bottom <= min)
            {
                DepthProjection.Y += distance.Bottom;
            }

        }

        //Move the monitor to not overlap
        if (!allowOverlaps && distance is { Left: < 0, Right: < 0, Top: < 0, Bottom: < 0 })
        {
            if (distance.Left > distance.Right && distance.Left > distance.Top && distance.Left > distance.Bottom)
            {
                DepthProjection.X -= distance.Left;
            }
            else if (distance.Right > distance.Top && distance.Right > distance.Bottom)
            {
                DepthProjection.X += distance.Right;
            }
            else if (distance.Top > distance.Bottom)
            {
                DepthProjection.Y -= distance.Top;
            }
            else DepthProjection.Y += distance.Bottom;
        }
    }

    public override string ToString() => $"{this.Id}";
}

