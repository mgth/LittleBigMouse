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

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Shapes;
using System.Windows.Threading;

using HLab.Base;
using HLab.Base.Wpf;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Extensions;
using HLab.Sys.Windows.API;

using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugin.Location.Plugins.Location.Anchors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Location.Plugins.Location;

class SizeScreenContentView : UserControl, IView<ViewModeScreenLocation, LocationScreenViewModel>, IViewScreenFrameTopLayer
{
}
/// <summary>
/// Logique d'interaction pour LocationScreenView.xaml
/// </summary>
partial class DefaultScreenView : UserControl, IView<ViewModeScreenLocation, LocationScreenViewModel>, IViewScreenFrameContent
{
    public DefaultScreenView()
    {
        InitializeComponent();
    }

    private Point? _staticPoint = null;

    private void SetStaticPoint()
    {
        var p = Mouse.GetPosition(this);
        _staticPoint = new Point(
            p.X / ActualWidth,
            p.Y / ActualHeight);
    }
    
    private void View_SizeChanged(object sender, EventArgs e)
    {
        if (_staticPoint != null)
        {
            Point p = PointToScreen(
                new Point(
                    ActualWidth * _staticPoint.Value.X,
                    ActualHeight * _staticPoint.Value.Y
                    ));

            User32.SetCursorPos((int)p.X, (int)p.Y);

            _staticPoint = null;
        }
    }

    private LocationScreenViewModel ViewModel => (DataContext as LocationScreenViewModel);
    private Panel MainPanel => this.FindVisualParent<IMultiScreensView>().GetMainPanel();
    private Panel BackPanel => this.FindVisualParent<IMultiScreensView>().GetMainPanel();



    private void OnMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        CaptureMouse();
        StartMove(e.GetPosition(MainPanel));
        //Gui.BringToFront(); // Todo
        e.Handled = true;
    }
    private void OnMouseLeftButtonUp(object sender, MouseButtonEventArgs e)
    {
        EndMove(e.GetPosition(MainPanel));
        ReleaseMouseCapture();
    }

    private void OnMouseMove(object sender, MouseEventArgs e)
    {
        if (!IsMouseCaptured) return;

        if (MainPanel == null) return;

        if (e.LeftButton != MouseButtonState.Pressed)
        {
            EndMove(e.GetPosition(MainPanel));
            ReleaseMouseCapture();
            return;
        }

        Move(e.GetPosition(MainPanel));
    }

    private Point _guiStartPosition;
    private Point _dragStartPosition;
    private Canvas _anchorsCanvas = null;

    public void StartMove(Point p)
    {
        _guiStartPosition = p;
        _dragStartPosition = new Point(ViewModel.Model.XMoving, ViewModel.Model.YMoving);

        ViewModel.Model.Moving = true;
    }


    public void EndMove(Point p)
    {

        if (_guiStartPosition.Equals(p)) ViewModel.Model.Selected = true;

        if (!ViewModel.Model.Moving) return;

        BackPanel.Children.Remove(_anchorsCanvas);
        _anchorsCanvas = null;

        ViewModel.Model.Moving = false;
        ViewModel.Model.Layout.Compact();
        //Todo : Plugin.ActivateConfig();
    }

    IMultiScreensView Presenter
    {
        get { return this.FindVisualParent<IMultiScreensView>(); }
    }


    public void Move(Point newGuiPosition)
    {
        if (_anchorsCanvas != null)
            BackPanel.Children.Remove(_anchorsCanvas);

        _anchorsCanvas = new Canvas
        {
            HorizontalAlignment = HorizontalAlignment.Left,
            VerticalAlignment = VerticalAlignment.Top
        };
        BackPanel.Children.Add(_anchorsCanvas);

        const double maxSnapDistance = 10.0;

        if (!this.GetFrame().ViewModel.Model.Moving) return;


        var ratioX = ViewModel.ScreenFrameViewModel.Presenter.VisualRatio.X;
        var ratioY = ViewModel.ScreenFrameViewModel.Presenter.VisualRatio.Y;

        var x0 = ViewModel.Model.Layout.X0;
        var y0 = ViewModel.Model.Layout.Y0;

        var dragOffset = (newGuiPosition - _guiStartPosition) / ratioX;

        var snapOffset = new Vector(double.PositiveInfinity, double.PositiveInfinity);

        var xAnchors = new List<Anchor>();
        var yAnchors = new List<Anchor>();

        /*var shift = */
        ShiftScreen(dragOffset);

        //use anchors when control key is not pressed
        if ((Keyboard.Modifiers & ModifierKeys.Control) == 0)
        {
            foreach (var other in ViewModel.Model.OtherScreens.Items)
            {
                foreach (var xAnchorThis in VerticalAnchors(ViewModel.Model, this.GetFrame().ViewModel.Model.XMoving - _dragStartPosition.X))
                {
                    foreach (var xAnchorOther in VerticalAnchors(other, 0))
                    {
                        var xOffset = xAnchorOther.Pos - xAnchorThis.Pos;

                        // if new offset is just egual to last, Add the new anchor visualization
                        if (Math.Abs(xOffset - snapOffset.X) < 0.01)
                        {
                            snapOffset.X = xOffset;
                            xAnchors.Add(xAnchorOther);
                            xAnchors.Add(new Anchor(xAnchorThis.Screen, xAnchorOther.Pos, xAnchorThis.Brush, xAnchorThis.StrokeDashArray));
                        }
                        // if new offset is better than old one, Remove all visuals and Add the new one
                        else if ((Math.Abs(xOffset) < Math.Abs(snapOffset.X)))
                        {
                            snapOffset.X = xOffset;
                            xAnchors.Clear();
                            xAnchors.Add(xAnchorOther);
                            xAnchors.Add(new Anchor(xAnchorThis.Screen, xAnchorOther.Pos, xAnchorThis.Brush, xAnchorThis.StrokeDashArray));
                        }
                    }
                }

                foreach (var yAnchorThis in HorizontalAnchors(ViewModel.Model, this.GetFrame().ViewModel.Model.YMoving - _dragStartPosition.Y))
                {
                    foreach (var yAnchorOther in HorizontalAnchors(other, 0))
                    {
                        var yOffset = yAnchorOther.Pos - yAnchorThis.Pos;
                        // if new offset is just egual to last, Add the new anchor visualization
                        if (Math.Abs(yOffset - snapOffset.Y) < 0.01)
                        {
                            snapOffset.Y = yOffset;
                            yAnchors.Add(yAnchorOther);
                            yAnchors.Add(new Anchor(yAnchorThis.Screen, yAnchorOther.Pos, yAnchorThis.Brush, yAnchorThis.StrokeDashArray));
                        }
                        // if new offset is better than old one, Remove all visuals and Add the new one
                        else if ((Math.Abs(yOffset) < Math.Abs(snapOffset.Y)))
                        {
                            snapOffset.Y = yOffset;
                            yAnchors.Clear();
                            yAnchors.Add(yAnchorOther);
                            yAnchors.Add(new Anchor(yAnchorThis.Screen, yAnchorOther.Pos, yAnchorThis.Brush, yAnchorThis.StrokeDashArray));
                        }
                    }
                }
            }


            //Apply offset if under maximal snap distance
            if (Math.Abs(snapOffset.X) > maxSnapDistance)
            {
                xAnchors.Clear();
                snapOffset.X = 0;
            }

            if (Math.Abs(snapOffset.Y) > maxSnapDistance)
            {
                yAnchors.Clear();
                snapOffset.Y = 0;
            }

            dragOffset += snapOffset;
        }

        /*shift = */
        ShiftScreen(dragOffset);

        foreach (var anchor in xAnchors)
        {
            var t = ReferenceEquals(anchor.Screen, ViewModel.Model) ? 5 : 2;
            var x = ratioX * (x0 + anchor.Pos);
            var l = ReferenceEquals(anchor.Screen, ViewModel.Model)
                ? new Line
                {
                    X1 = x,
                    X2 = x,
                    Y1 = this.GetFrame().Margin.Top,
                    Y2 = this.GetFrame().Margin.Top + this.GetFrame().ActualHeight,
                        //StrokeThickness = 4,
                        Stroke = anchor.Brush,
                    Effect = new DropShadowEffect
                    {
                        Color = Colors.Transparent,
                        BlurRadius = 20,
                        Opacity = 1,
                        ShadowDepth = 0,

                    },
                    StrokeDashArray = anchor.StrokeDashArray
                }
                : new Line
                {
                    X1 = x,
                    X2 = x,
                    Y1 = ratioY * (y0 + anchor.Screen.InMm.OutsideY),
                    Y2 = ratioY * (y0 + anchor.Screen.InMm.OutsideBounds.Bottom),
                        //StrokeThickness = 2,
                        Stroke = anchor.Brush,
                    StrokeDashArray = anchor.StrokeDashArray,
                    Opacity = 0.6
                };
            _anchorsCanvas.Children.Add(l);
        }

        foreach (var anchor in yAnchors)
        {
            var y = ratioY * (y0 + anchor.Pos);

            var l = ReferenceEquals(anchor.Screen, ViewModel.Model) ?
                new Line
                {
                    Y1 = y,
                    Y2 = y,
                    X1 = this.GetFrame().Margin.Left,//0,
                        X2 = this.GetFrame().Margin.Left + this.GetFrame().ActualWidth,//this.FindParent<MultiScreensView>().BackgroundGrid.ActualWidth,
                        Stroke = anchor.Brush,
                        //StrokeThickness = 4,
                        //StrokeDashArray = new DoubleCollection { 5, 3 }
                        Effect = new DropShadowEffect
                    {
                        Color = Colors.Transparent,
                        BlurRadius = 20,
                        Opacity = 1,
                        ShadowDepth = 0,

                    },
                    StrokeDashArray = anchor.StrokeDashArray
                } :
                new Line
                {
                    Y1 = y,
                    Y2 = y,
                    X1 = ratioX * (x0 + anchor.Screen.InMm.OutsideX),//0,
                        X2 = ratioX * (y0 + anchor.Screen.InMm.OutsideBounds.Right),//this.FindParent<MultiScreensView>().BackgroundGrid.ActualWidth,
                        Stroke = anchor.Brush,
                        //StrokeThickness = 2,
                        StrokeDashArray = anchor.StrokeDashArray,
                    Opacity = 0.6,
                    Effect = new DropShadowEffect
                    {
                        Color = Colors.White,
                        BlurRadius = 20,
                        Opacity = 1,
                        ShadowDepth = 0,
                    },
                };

            _anchorsCanvas.Children.Add(l);
        }
    }
    private void ShiftScreen(Vector offset)
    {
        Point pos = _dragStartPosition + offset;

        ViewModel.Model.XMoving = pos.X;
        ViewModel.Model.YMoving = pos.Y;

    }

    public List<Anchor> VerticalAnchors(Monitor s, double shift) => new List<Anchor>
        {
            new Anchor(s,s.InMm.OutsideBounds.X + shift,new SolidColorBrush(Colors.Chartreuse),new DoubleCollection{25,2}),
            new Anchor(s,s.InMm.X + shift,new SolidColorBrush(Colors.LightGreen),new DoubleCollection{25,0}),
            new Anchor(s,s.InMm.X + shift + s.InMm.Width /2,new SolidColorBrush(Colors.Red),new DoubleCollection{20,7,2,7}),
            new Anchor(s,s.InMm.Bounds.Right + shift,new SolidColorBrush(Colors.LightGreen),new DoubleCollection{25,0}),
            new Anchor(s,s.InMm.OutsideBounds.Right + shift,new SolidColorBrush(Colors.Chartreuse),new DoubleCollection{25,2}),
        };

    public List<Anchor> HorizontalAnchors(Monitor s, double shift) => new List<Anchor>
        {
            new Anchor(s,s.InMm.OutsideBounds.Y + shift,new SolidColorBrush(Colors.Chartreuse),new DoubleCollection{25,2}),
            new Anchor(s,s.InMm.Y  + shift,new SolidColorBrush(Colors.LightGreen),new DoubleCollection{25,0}),
            new Anchor(s,s.InMm.Y + shift + s.InMm.Height /2,new SolidColorBrush(Colors.Red),new DoubleCollection{20,7,2,7}),
            new Anchor(s,s.InMm.Bounds.Bottom + shift,new SolidColorBrush(Colors.LightGreen),new DoubleCollection{25,0}),
            new Anchor(s,s.InMm.OutsideBounds.Bottom + shift,new SolidColorBrush(Colors.Chartreuse),new DoubleCollection{25,2}),
        };

    private void OnKeyEnterUpdate(object sender, KeyEventArgs e)
    {
        ViewHelper.OnKeyEnterUpdate(sender, e);
    }

    private static double WheelDelta(MouseWheelEventArgs e)
    {
        double delta = (e.Delta > 0) ? 1 : -1;
        if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
        if ((Keyboard.Modifiers & ModifierKeys.Shift) != 0) delta *= 10;
        return delta;
    }

    private void OnMouseWheel(object sender, MouseWheelEventArgs e)
    {
        if (sender is TextBox tb)
        {
            var p = e.GetPosition(tb);
            var rx = p.X / tb.ActualWidth;
            var ry = p.Y / tb.ActualHeight;

            var delta = WheelDelta(e);

            tb.SetBindingValue(TextBox.TextProperty, v =>
            {
                if (v is string s)
                {
                    if (double.TryParse(s, NumberStyles.AllowDecimalPoint, CultureInfo.InvariantCulture, out var d))
                    {
                        return (d + delta).ToString(CultureInfo.InvariantCulture);
                    }
                }
                return v;
            });

            ViewModel.Model.Layout.Compact();

            Dispatcher.BeginInvoke(() =>
            {
                var p2 = new Point(rx * tb.ActualWidth, ry * tb.ActualHeight);
                var l = tb.PointToScreen(p2);
                HLab.Sys.Windows.API.User32.SetCursorPos((int)l.X, (int)l.Y);
            }, DispatcherPriority.Loaded);
        }
    }

    private void OnMouseDoubleClick(object sender, MouseButtonEventArgs e)
    {
        if (sender is TextBox tb)
        {
            tb.SetBindingValue(TextBox.TextProperty, v => (100.0).ToString(CultureInfo.InvariantCulture));
        }
    }

    private Point _captureLocation;
    private double _startValue;
    private void TextBox_MouseDown(object sender, MouseButtonEventArgs e)
    {
        Debug.WriteLine("down");
        if (sender is TextBox tb)
        {
            if(tb.CaptureMouse())
            {
                _captureLocation = e.GetPosition(tb);
                if(double.TryParse(tb.Text, out var value))
                {
                    _startValue = value;
                    e.Handled = true;
                }
                else
                {
                    tb.ReleaseMouseCapture();
                }
            }
        }
    }

    private void TextBox_MouseMove(object sender, MouseEventArgs e)
    {
        if (sender is TextBox tb)
        {
            if(tb.IsMouseCaptured)
            {
                Debug.WriteLine("move");
                var vector = e.GetPosition(tb) - _captureLocation;
                var value  = _startValue + vector.Y / 5;
                //Debug.WriteLine(value);
                Debug.WriteLine(vector);
                //tb.SetBindingValue(TextBox.TextProperty, v => value.ToString(CultureInfo.InvariantCulture));
            }
        }
    }

    private void TextBox_MouseUp(object sender, MouseButtonEventArgs e)
    {
        Debug.WriteLine("up");
        if (sender is TextBox tb && tb.IsMouseCaptured)
        {
            tb.ReleaseMouseCapture();
        }
    }

}
