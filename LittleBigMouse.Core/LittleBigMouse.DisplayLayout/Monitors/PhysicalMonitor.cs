/*
  LittleBigMouse.DisplayLayout
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.DisplayLayout.

    LittleBigMouse.DisplayLayout is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.DisplayLayout is distributed in the hope that it will be useful,
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

        // Which borders the geometry is built from, and how this monitor comes to own them:
        // see MonitorBorderPolicy. The whole rotation / projection / zones chain below just
        // follows the size it publishes, and never asks which mode produced it.
        _borderPolicy = new MonitorBorderPolicy(model.PhysicalSize, layout.Options).DisposeWith(this);
        Borders = _borderPolicy.Borders;

        // Republish the policy's ownership flag as this monitor's own property change:
        // persistence and the UI watch PhysicalMonitor, not the policy behind it.
        _borderPolicy.Changing
            .Where(e => e.PropertyName == nameof(MonitorBorderPolicy.Customized))
            .Subscribe(_ => this.RaisePropertyChanging(nameof(BordersCustomized)))
            .DisposeWith(this);

        _borderPolicy.Changed
            .Where(e => e.PropertyName == nameof(MonitorBorderPolicy.Customized))
            .Subscribe(_ => this.RaisePropertyChanged(nameof(BordersCustomized)))
            .DisposeWith(this);

        // DisplayBorders is not ISavable so UnsavedOn cannot track it.
        // Load() resets Saved=true at its end, so loading does not leave a dirty flag.
        _borderPolicy.BordersChanged
            .Subscribe(_ => Saved = false)
            .DisposeWith(this);

        var effectiveSizeObs = _borderPolicy.EffectiveSize;

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
                (physicalSize, orientation, ratio) => physicalSize.Rotate(orientation).Scale(ratio).Locate()
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
    }

    readonly MonitorBorderPolicy _borderPolicy;

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
    /// rotation / depth-projection / zones chain reads through here. Either the shared per-model
    /// <see cref="PhysicalMonitorModel.PhysicalSize"/> or, when "Border values" is "PerMonitor",
    /// the same size with this monitor's own borders substituted; <see cref="MonitorBorderPolicy"/>
    /// decides which and re-publishes on a mode switch, so no geometry consumer sees the option.
    /// </summary>
    public IMutableDisplaySize EffectivePhysicalSize => _effectivePhysicalSize.Value;
    readonly ObservableAsPropertyHelper<IMutableDisplaySize> _effectivePhysicalSize;

    /// <summary>
    /// This monitor's own bezel borders, used to build the geometry only when "Border values" is
    /// "PerMonitor". Seeded from the model at construction; loaded from / saved to the monitor's own
    /// registry key. In "PerModel" mode the geometry ignores these and uses the shared model borders.
    /// </summary>
    /// <remarks>Held by <see cref="MonitorBorderPolicy"/>; kept in a field here so consumers still
    /// observe it through a plain get-only property on the monitor.</remarks>
    public DisplayBorders Borders { get; }

    /// <summary>
    /// True once this monitor owns its bezel borders (persisted values were loaded,
    /// or a border was edited in "PerMonitor" mode). Until then <see cref="Borders"/>
    /// mirrors the shared model values, and nothing per-monitor is persisted.
    /// </summary>
    /// <remarks>The state lives in <see cref="MonitorBorderPolicy"/>, which also sets it on its
    /// own when it detects a user edit; the constructor republishes those changes here.</remarks>
    public bool BordersCustomized
    {
       get => _borderPolicy.Customized;
       set => _borderPolicy.Customized = value;
    }

    /// <summary>
    /// Dimensions with rotation applied
    /// </summary>
    [DataMember] public IMutableDisplaySize PhysicalRotated
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
    public IMutableDisplaySize DepthProjection
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
    public IMutableDisplaySize DepthProjectionUnrotated => _depthProjectionUnrotated.Value;
    readonly ObservableAsPropertyHelper<IMutableDisplaySize> _depthProjectionUnrotated;

    /// <summary>
    /// Final ratio to deal with monitor distance
    /// </summary>
    [DataMember]
    public IMutableDisplayRatio DepthRatio { get; }

    [DataMember]
    public BorderResistance BorderResistance { get; } = new BorderResistance();

    /// <summary>
    /// Diagonal
    /// </summary>
    public double Diagonal => _diagonal.Value;
    readonly ObservableAsPropertyHelper<double> _diagonal;

    public override string ToString() => $"{this.Id}";
}
