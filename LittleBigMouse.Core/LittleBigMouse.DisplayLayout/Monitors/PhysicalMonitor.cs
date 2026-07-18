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
using System.ComponentModel;
using System.Reactive.Concurrency;
using System.Linq;
using System.Reactive.Linq;
using System.Runtime.Serialization;
using System.Text.Json.Serialization;
using DynamicData;
using HLab.Base.ReactiveUI;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;


namespace LittleBigMouse.DisplayLayout.Monitors;

[DataContract]
public class PhysicalMonitor : SavableReactiveModel
{
    public class Design : PhysicalMonitor
    {
        public Design() : base("PNP0000", MonitorsLayout.MonitorsLayoutDesign, PhysicalMonitorModel.Design)
        {
            //if(!Avalonia.Controls.Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
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
       get;
       set => SetUnsavedValue(ref field, value);
    }

    [DataMember]
    public string DeviceId
    {
       get;
       set => SetUnsavedValue(ref field, value);
    }

    public PhysicalMonitor(string id, IMonitorsLayout layout, PhysicalMonitorModel model)
    {
        Id = id;
        Layout = layout;
        Model = model;

        Sources.DisposeWith(this);

        Sources.Connect()
            .AutoRefresh(e => e.Saved)
            .ToCollection()
            .Do(ParseDisplaySources)
            .Subscribe().DisposeWith(this);

        // Per-monitor border source, seeded from the model so "PerMonitor" starts matching "PerModel".
        Borders = new DisplayBorders
        {
            Left = model.PhysicalSize.LeftBorder,
            Top = model.PhysicalSize.TopBorder,
            Right = model.PhysicalSize.RightBorder,
            Bottom = model.PhysicalSize.BottomBorder,
        };
        _borderOverride = new DisplayBorderOverride(model.PhysicalSize, Borders);

        // Observe BorderValues directly via ILayoutOptions.PropertyChanged — WhenAnyValue through
        // IMonitorsLayout (which declares no INotifyPropertyChanged) loses the innermost subscription.
        var borderValuesObs = Observable
            .FromEventPattern<PropertyChangedEventHandler, PropertyChangedEventArgs>(
                h => Layout.Options.PropertyChanged += h,
                h => Layout.Options.PropertyChanged -= h)
            .Where(e => e.EventArgs.PropertyName == nameof(ILayoutOptions.BorderValues))
            .Select(_ => Layout.Options.BorderValues)
            .StartWith(Layout.Options.BorderValues)
            .DistinctUntilChanged();

        // The size the geometry is built from: the shared per-model size, or — when "Border values"
        // is "PerMonitor" — the same size with this monitor's own borders substituted. Reactive so a
        // mode switch rewires rotation / projection / zones live.
        // Shared, replayed stream — Replay(1) ensures _physicalRotated and _depthProjectionUnrotated
        // receive the current value when they subscribe, without missing the StartWith emission that
        // already fired for _effectivePhysicalSize. Direct subscription also avoids the
        // WhenAnyValue(e => e.EffectivePhysicalSize) OAPH-PropertyChanged round-trip that breaks
        // in Avalonia's synchronous reentrancy model (mode-change event → OAPH fires → inner
        // PropertyChanged → WhenAnyValue chain stops).
        var effectiveSizeObs = borderValuesObs
            .Select(mode => mode == "PerMonitor" ? (IDisplaySize)_borderOverride : Model.PhysicalSize)
            .Replay(1)
            .RefCount();

        _effectivePhysicalSize = effectiveSizeObs
            .ToProperty(this, e => e.EffectivePhysicalSize, initialValue: model.PhysicalSize, scheduler: Scheduler.Immediate)
            .DisposeWith(this);


        effectiveSizeObs
            .CombineLatest(
                this.WhenAnyValue(e => e.ActiveSource.Source.Orientation),
                (physicalSize, orientation) => physicalSize.Rotate(orientation)
            )
            .Subscribe(r => PhysicalRotated = r)
            .DisposeWith(this);

        //RemainingPhysicalMonitors = Layout.PhysicalMonitors.Items.AsObservableChangeSet().Filter(s => !Equals(s, this)).AsObservableList();

        DepthRatio = new DisplayRatioValue(1.0, 1.0);

        // Use Subscribe + ReferenceEquals setter instead of ToProperty to bypass
        // DistinctUntilChanged(EqualityComparer<IDisplaySize>.Default), which would suppress
        // the PerModel→PerMonitor switch when both DPs have identical values (borders not yet loaded).
        effectiveSizeObs
            .CombineLatest(
                this.WhenAnyValue(e => e.ActiveSource.Source.Orientation),
                this.WhenAnyValue(e => e.DepthRatio),
                (physicalSize, orientation, ratio) => (IDisplaySize)physicalSize.Rotate(orientation).Scale(ratio).Locate()
            )
            .Log(this, "_inMm")
            .Subscribe(dp => DepthProjection = dp)
            .DisposeWith(this);

        _depthProjectionUnrotated = effectiveSizeObs
            .CombineLatest(
                this.WhenAnyValue(e => e.DepthRatio),
                (physicalSize, ratio) => physicalSize.Scale(ratio)
            ).Log(this, "_inMmU").ToProperty(this, e => e.DepthProjectionUnrotated, scheduler: Scheduler.Immediate).DisposeWith(this);

        _diagonal = this.WhenAnyValue(
            e => e.DepthProjection.Height,
            e => e.DepthProjection.Width,
            (h, w) => Math.Sqrt(w * w + h * h)
            ).Log(this, "_diagonal").ToProperty(this, e => e.Diagonal).DisposeWith(this);

        this.UnsavedOn(
            e => e.Model,
            e => e.DepthProjection,
            e => e.DepthRatio,
            e => e.BorderResistance
        );

        // DisplayBorders is not ISavable so UnsavedOn cannot track it.
        // Skip(1) drops the initial combined emission at subscription time.
        // Load() resets Saved=true at its end, so loading does not leave a dirty flag.
        this.WhenAnyValue(
                e => e.Borders.Left,
                e => e.Borders.Top,
                e => e.Borders.Right,
                e => e.Borders.Bottom,
                (l, t, r, b) => (l, t, r, b))
            .Skip(1)
            .Subscribe(_ => Saved = false)
            .DisposeWith(this);
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
       get;
       set => this.SetUnsavedValue(ref field, value);
    }

    /// <summary>
    /// Keep this monitor out of the mouse layout: it stays attached to the desktop
    /// but gets no zone, so the cursor treats it as a wall. For displays that are
    /// not really monitors — water-cooling pump LCDs, sensor panels… (#504)
    /// </summary>
    [DataMember]
    public bool ExcludedFromLayout
    {
       get;
       set => this.SetUnsavedValue(ref field, value);
    }

    /// <summary>
    /// Serial number from EDID
    /// </summary>
    [DataMember]
    public string SerialNumber
    {
       get;
       set => this.SetUnsavedValue(ref field, value);
    }

    /// <summary>
    /// True when placement has been set by user or by automatic placement
    /// </summary>
    public bool Placed
    {
       get;
       set => this.RaiseAndSetIfChanged(ref field, value);
    }

    /// <summary>
    /// Monitor model
    /// </summary>
    public PhysicalMonitorModel Model { get; }

    /// <summary>
    /// The physical size (with bezel borders) this monitor's geometry is built from — the whole
    /// rotation / depth-projection / zones chain reads through here. Today it always returns the
    /// shared per-model <see cref="PhysicalMonitorModel.PhysicalSize"/>, so behaviour is unchanged.
    /// It exists as the single seam where a future "Border values: per monitor" option can substitute
    /// a per-monitor border source, without touching any geometry consumer. See the border-values plan.
    /// </summary>
    public IDisplaySize EffectivePhysicalSize => _effectivePhysicalSize.Value;
    readonly ObservableAsPropertyHelper<IDisplaySize> _effectivePhysicalSize;

    /// <summary>
    /// This monitor's own bezel borders, used to build the geometry only when "Border values" is
    /// "PerMonitor". Seeded from the model at construction; loaded from / saved to the monitor's own
    /// registry key. In "PerModel" mode the geometry ignores these and uses the shared model borders.
    /// </summary>
    public DisplayBorders Borders { get; }
    readonly DisplayBorderOverride _borderOverride;

    /// <summary>
    /// Dimensions with rotation applied
    /// </summary>
    [DataMember] public IDisplaySize PhysicalRotated
    {
        get;
        private set
        {
            if (ReferenceEquals(field, value)) return;
            this.RaisePropertyChanging();
            field = value;
            this.RaisePropertyChanged();
        }
    }


    // Mm

    /// <summary>
    /// Dimensions with depth ratio applied to deal with monitor distance
    /// </summary>
    [DataMember]
    public IDisplaySize DepthProjection
    {
        get;
        private set
        {
            if (ReferenceEquals(field, value)) return;
            // Carry the layout-computed position forward so monitors don't collapse to 0,0
            // when a mode switch replaces the DP object with a fresh one.
            if (field is not null && value is not null)
            {
                value.X = field.X;
                value.Y = field.Y;
            }
            this.RaisePropertyChanging();
            field = value;
            this.RaisePropertyChanged();
        }
    }

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

    [DataMember]
    public BorderResistance BorderResistance { get; } = new BorderResistance();

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

