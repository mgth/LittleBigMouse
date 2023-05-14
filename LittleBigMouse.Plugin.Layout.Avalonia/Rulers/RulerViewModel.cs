/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

public class RulerViewModel : ViewModel
{
    public PhysicalMonitor Monitor { get; }
    public int Orientation { get; }
    public PhysicalSource DrawOn { get; }

    public bool Vertical { get; }
    public bool Horizontal { get; }
    public bool Revert { get; }

    public RulerViewModel(PhysicalMonitor monitor, PhysicalSource drawOn, int orientation)
    {
        Orientation = orientation;
        switch (Orientation)
        {
            case 0:
                Vertical = false;
                Horizontal = true;
                Revert = false;
                break;

            case 1:
                Vertical = true;
                Horizontal = false;
                Revert = true;
                break;

            case 2:
                Vertical = false;
                Horizontal = true;
                Revert = true;
                break;

            case 3:
                Vertical = true;
                Horizontal = false;
                Revert = false;
                break;

            default:
                throw new ArgumentOutOfRangeException(nameof(Orientation), orientation, null);
        }
        Monitor = monitor;
        DrawOn = drawOn;

        _zeroX = this.WhenAnyValue(
                e => e.Monitor.DepthProjection.X, //Todo : retrieve moving location 
                e => e.DrawOn.Monitor.DepthProjection.X, 

                (monitorX,drawOnX) => monitorX - drawOnX)

            .ToProperty(this, e => e.ZeroX);

        _zeroY = this.WhenAnyValue(
                e => e.Monitor.DepthProjection.Y,
                e => e.DrawOn.Monitor.DepthProjection.Y, 

                (monitorY,drawOnY) => monitorY - drawOnY)

            .ToProperty(this, e => e.ZeroY);

        _rulerLength = this.WhenAnyValue(
            e => e.Monitor.DepthProjection.Width,
            e => e.Monitor.DepthProjection.Height, 

            (width,height) => Vertical ? height : width)

            .ToProperty(this, e => e.RulerLength);

        _rulerStart = this.WhenAnyValue(
            e => e.DrawOn.Monitor.DepthProjection.X, // todo : should be moving locations
            e => e.DrawOn.Monitor.DepthProjection.Y,
            e => e.Monitor.DepthProjection.X,
            e => e.Monitor.DepthProjection.Y,

            (drawOnX, drawOnY, monitorX, monitorY) 
                => Vertical ? drawOnY - monitorY : drawOnX - monitorX)

            .ToProperty(this, e => e.RulerStart);

        _rulerEnd = this.WhenAnyValue(
            e => e.RulerStart,
            e => e.RulerLength, 

            (start,length) => start + length)

            .ToProperty(this, e => e.RulerEnd);

        _length = this.WhenAnyValue(
             e => e.DrawOn.Monitor.DepthProjection.Width,
             e => e.DrawOn.Monitor.DepthProjection.Height, 

             (width,height) => Vertical ? height : width)

            .ToProperty(this, e => e.Length);

        _size = this.WhenAnyValue(
            e => e.DrawOn.Monitor.DepthProjection.Width,
            e => e.DrawOn.Monitor.DepthProjection.Height, 

            (width,height) => Vertical ? width : height)

            .ToProperty(this, e => e.Size);

        _zero = this.WhenAnyValue(
            e => e.ZeroX,
            e => e.ZeroY, 

            (zeroX,zeroY) => Vertical ? zeroY : zeroX)

            .ToProperty(this, e => e.Zero);

        Selected = ReferenceEquals(DrawOn, Monitor);
    }

    public bool Enabled
    {
        get => _enable;
        set => this.RaiseAndSetIfChanged(ref _enable, value);
    }
    bool _enable;

    public bool Selected
    {
        get => _selected;
        set => this.RaiseAndSetIfChanged(ref _selected, value);
    }
    bool _selected;

    public double ZeroX => _zeroX.Value;
    readonly ObservableAsPropertyHelper<double> _zeroX;

    public double ZeroY => _zeroY.Value;
    readonly ObservableAsPropertyHelper<double> _zeroY;

    public double RulerLength => _rulerLength.Value;
    readonly ObservableAsPropertyHelper<double> _rulerLength;


    public double RulerEnd => _rulerEnd.Value;
    readonly ObservableAsPropertyHelper<double> _rulerEnd;




    /// <summary>
    /// Calculate the first tick value of the ruler
    /// </summary>
    public double RulerStart => _rulerStart.Value;
    readonly ObservableAsPropertyHelper<double> _rulerStart;

    public double Length => _length.Value;
    readonly ObservableAsPropertyHelper<double> _length;


    public double Size => _size.Value;
    readonly ObservableAsPropertyHelper<double> _size;

    public double Zero => _zero.Value;
    readonly ObservableAsPropertyHelper<double> _zero;
}
