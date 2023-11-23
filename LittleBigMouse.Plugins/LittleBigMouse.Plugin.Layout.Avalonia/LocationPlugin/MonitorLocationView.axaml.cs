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

using System.Diagnostics;
using Avalonia;
using Avalonia.Collections;
using Avalonia.Controls;
using Avalonia.Input;
using Avalonia.Interactivity;
using Avalonia.Media;
using Avalonia.Threading;
using HLab.Base.Avalonia.Controls;
using HLab.Mvvm.Annotations;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin.Anchors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Avalonia;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;

using static HLab.Sys.Windows.API.WinUser;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

public static class MonitorAnchorsExtensions
{
    static readonly AvaloniaList<double>? InsideDash = null;
    static readonly AvaloniaList<double> OutsideDash = new() { 25, 2 };
    static readonly AvaloniaList<double> MiddleDash = new() { 20, 7, 2, 7 };


    public static Anchor GetAnchor(this PhysicalMonitor monitor, double pos, Brush brush, AvaloniaList<double> dashArray)
        => new(monitor, pos, brush, dashArray);

    public static Anchor GetOutsideAnchor(this PhysicalMonitor monitor, double pos)
        => new(monitor, pos,  Brushes.Chartreuse, OutsideDash);

    public static Anchor GetInsideAnchor(this PhysicalMonitor monitor, double pos)
        => new(monitor, pos, new SolidColorBrush(Colors.LightGreen), InsideDash);

    public static Anchor GetMiddleAnchor(this PhysicalMonitor monitor, double pos)
        => new(monitor, pos, Brushes.Red, MiddleDash);

}


internal partial class MonitorLocationView : UserControl, IView<MonitorLocationViewMode, MonitorLocationViewModel>, IMonitorFrameContentViewClass
{
    public MonitorLocationView()
    {
        InitializeComponent();
    }

    protected override void OnUnloaded(RoutedEventArgs e)
    {
        if (DataContext is MonitorLocationViewModel viewModel)
        {
            viewModel.Ruler = false;
        }

        base.OnUnloaded(e);
    }

    Point? _staticPoint = null;

    //void SetStaticPoint()
    //{
    //    var p = Pointer.GetPosition(this);
    //    _staticPoint = new Point(
    //        p.X / Bounds.Width,
    //        p.Y / Bounds.Height);
    //}

    void View_SizeChanged(object? sender, SizeChangedEventArgs e)
    {
        if (_staticPoint != null)
        {
            var p = this.PointToScreen(
                new Point(
                    Bounds.Width * _staticPoint.Value.X,
                    Bounds.Height * _staticPoint.Value.Y
                ));

            SetCursorPos((int)p.X, (int)p.Y);

            _staticPoint = null;
        }
    }

    MonitorLocationViewModel ViewModel => DataContext as MonitorLocationViewModel;
    Panel? MainPanel => this.GetPresenter()?.MainPanel;

    IFrameLocation? _frameLocation;
    FrameMover? _frameMover;

    protected override void OnPointerPressed(PointerPressedEventArgs e)
    {
        base.OnPointerPressed(e);

        var model = ViewModel?.Model;
        if(model == null) return;

        var layout = this.GetLayout();
        if(layout == null) return;

        var frame = this.GetMonitorFrame();
        if(frame == null) return;

        var presenter = this.GetPresenter();
        if(presenter == null) return;

        var panel = MainPanel;
        if(panel == null) return;

        e.Pointer.Capture(this);

        _frameLocation = this.GetFrameLocation();

        _frameMover = new FrameMover(
            model, 
            layout, 
            frame, 
            panel, 
            e.GetPosition(panel),
            presenter
            );

        this.SetFrameLocation(_frameMover);

        //Gui.BringToFront(); // Todo
        e.Handled = true;
    }

    protected override void OnPointerReleased(PointerReleasedEventArgs e)
    {
        base.OnPointerReleased(e);
        EndMove(e);
    }

    protected void EndMove(PointerEventArgs e)
    {
        _frameMover?.EndMove(e.GetPosition(MainPanel));
        _frameMover = null;
        // restore frame location to normal behavior
        this.SetFrameLocation(_frameLocation);

        e.Pointer.Capture(null);
    }


    protected override void OnPointerMoved(PointerEventArgs e)
    {
        base.OnPointerMoved(e);

        if (e.Pointer.Captured == null) return;

        // if button not pressed any more, end moving.
        if (!e.GetCurrentPoint(this).Properties.IsLeftButtonPressed)
        {
            EndMove(e);
            return;
        }

        //move monitor, checking control key to disable anchors snapping
        _frameMover?.Move(e.GetPosition(MainPanel),(e.KeyModifiers & KeyModifiers.Control) == 0);
    }

    void OnKeyEnterUpdate(object sender, KeyEventArgs e)
    {
        // TODO : Avalonia
        // ViewHelper.OnKeyEnterUpdate(sender, e);
    }

    static double WheelDelta(PointerWheelEventArgs e)
    {
        double delta = (e.Delta.Y > 0) ? 1 : -1;
        if ((e.KeyModifiers & KeyModifiers.Control) != 0) delta /= 10;
        if ((e.KeyModifiers & KeyModifiers.Shift) != 0) delta *= 10;
        return delta;
    }

    void OnMouseWheel(object sender, PointerWheelEventArgs e)
    {
        if (sender is not DoubleBox db) return;

        var p = e.GetPosition(db);
        var rx = p.X / db.Bounds.Width;
        var ry = p.Y / db.Bounds.Height;

        db.Value += WheelDelta(e);

        this.GetLayout()?.Compact();

        Dispatcher.UIThread.InvokeAsync(() => 
        {
            var p2 = new Point(rx * db.Bounds.Width, ry * db.Bounds.Height);
            var l = db.PointToScreen(p2);
            SetCursorPos((int)l.X, (int)l.Y);
        }, DispatcherPriority.Loaded);
    }

    void OnMouseDoubleClick(object sender, PointerPressedEventArgs e)
    {
        if (sender is TextBox tb)
        {
            // TODO : Avalonia
            //tb.SetBindingValue(TextBox.TextProperty, v => (100.0).ToString(CultureInfo.InvariantCulture));
        }
    }

    Point _captureLocation;
    double _startValue;

    void TextBox_MouseDown(object sender, PointerPressedEventArgs e)
    {
        Debug.WriteLine("down");
        if (sender is not TextBox tb) return;

        e.Pointer.Capture(tb);

        _captureLocation = e.GetPosition(tb);

        if(double.TryParse(tb.Text, out var value))
        {
            _startValue = value;
            e.Handled = true;
        }
        else
        {
            e.Pointer.Capture(null);
        }
    }

    void TextBox_PointerMove(object? sender, PointerEventArgs e)
    {
        if (sender is not TextBox tb) return;

        if (e.Pointer.Captured != sender) return;

        Debug.WriteLine("move");
        var vector = e.GetPosition(tb) - _captureLocation;
        var value  = _startValue + vector.Y / 5;
        //Debug.WriteLine(value);
        Debug.WriteLine(vector);
        //tb.SetBindingValue(TextBox.TextProperty, v => value.ToString(CultureInfo.InvariantCulture));
    }

    void TextBox_PointerReleased(object sender, PointerReleasedEventArgs e)
    {
        Debug.WriteLine("up");
        if (sender is TextBox tb && e.Pointer.Captured == tb)
        {
            e.Pointer.Capture(null);
        }
    }

}