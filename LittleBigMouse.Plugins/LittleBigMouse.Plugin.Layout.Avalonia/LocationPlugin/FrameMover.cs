using Avalonia;
using Avalonia.Controls;
using Avalonia.Controls.Shapes;
using Avalonia.Media;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin.Anchors;
using LittleBigMouse.Plugins.Avalonia;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;

public class FrameMover : ReactiveObject, IFrameLocation
{
    public FrameMover(
        PhysicalMonitor monitor, 
        IMonitorsLayout layout, 
        IMonitorFrameView frame,
        Panel backPanel, 
        Point startPosition, 
        IMonitorsLayoutPresenterView presenter)
    {
        _monitor = monitor;
        _layout = layout; 
        _frame = frame;
        _backPanel = backPanel;

        _guiStartPosition = startPosition;
        _presenter = presenter;

        _width = this
            .WhenAnyValue(e => e._monitor.PhysicalRotated.OutsideWidth)
            .ToProperty(this,e => e.Width);

        _height = this
            .WhenAnyValue(e => e._monitor.PhysicalRotated.OutsideHeight)
            .ToProperty(this,e => e.Height);

        X = _monitor.DepthProjection.X;
        Y = _monitor.DepthProjection.Y;

        _dragStartPosition = new Point( X,  Y );
    }

    readonly PhysicalMonitor _monitor;
    readonly IMonitorsLayout _layout;
    readonly IMonitorFrameView _frame;
    readonly Panel _backPanel;
    readonly IMonitorsLayoutPresenterView _presenter;

    public double Width => _width.Value;
    readonly ObservableAsPropertyHelper<double> _width;

    public double Height => _height.Value;
    readonly ObservableAsPropertyHelper<double> _height;

    public double X
    {
        get => _x;
        set => this.RaiseAndSetIfChanged(ref _x, value);
    }
    double _x;

    public double Y
    {
        get => _y;
        set => this.RaiseAndSetIfChanged(ref _y, value);
    }
    double _y;

    public static List<Anchor> VerticalAnchors(PhysicalMonitor monitor, double shift)
    {
        var s = monitor.DepthProjection;
        return new List<Anchor>
        {
            monitor.GetOutsideAnchor(s.OutsideBounds.X + shift),
            monitor.GetInsideAnchor(s.X + shift),
            monitor.GetMiddleAnchor(s.X + shift + s.Width / 2),
            monitor.GetInsideAnchor(s.Bounds.Right + shift),
            monitor.GetOutsideAnchor(s.OutsideBounds.Right + shift),
        };
    }

    public List<Anchor> HorizontalAnchors(PhysicalMonitor monitor, double shift)
    {
        var s = monitor.DepthProjection;
        return new List<Anchor>
        {
            monitor.GetOutsideAnchor(s.OutsideBounds.Y + shift),
            monitor.GetInsideAnchor(s.Y + shift),
            monitor.GetMiddleAnchor(s.Y + shift + s.Height / 2),
            monitor.GetInsideAnchor(s.Bounds.Bottom + shift),
            monitor.GetOutsideAnchor(s.OutsideBounds.Bottom + shift),
        };
    }


    readonly Point _guiStartPosition;
    readonly Point _dragStartPosition;
    Canvas? _anchorsCanvas = null;

    void ShiftMonitor(Vector offset)
    {
        var pos = _dragStartPosition + offset;

        X = pos.X;
        Y = pos.Y;
    }


    public void EndMove(Point p)
    {
        if (_guiStartPosition == p)
        {
            _presenter.ViewModel.SelectedMonitor = _frame.ViewModel;
        }

        if(_anchorsCanvas is {} canvas)
            _backPanel.Children.Remove(canvas);

        _anchorsCanvas = null;

        if (_monitor.Sources.Items.Any(s => s.Source.Primary))
        {
            foreach (var monitor in _layout.PhysicalMonitors.Except([_monitor]))
            {
                monitor.DepthProjection.X -= X;
                monitor.DepthProjection.Y -= Y;
            }
        }
        else
        {
            _monitor.DepthProjection.X = X;
            _monitor.DepthProjection.Y = Y;
        }

        //Remove gaps 
        _layout.Compact();

        _layout.UpdatePhysicalMonitors();

        //Todo : Plugin.ActivateConfig();
    }

    public void Move(Point newGuiPosition, bool useAnchors)
    {
        // if an anchor canvas exists remove it
        if (_anchorsCanvas != null)
            _backPanel.Children.Remove(_anchorsCanvas);

        _anchorsCanvas = new Canvas
        {
            HorizontalAlignment = global::Avalonia.Layout.HorizontalAlignment.Left,
            VerticalAlignment = global::Avalonia.Layout.VerticalAlignment.Top
        };
        _backPanel.Children.Add(_anchorsCanvas);

        const double maxSnapDistance = 10.0;

        var ratioX = _presenter.GetRatio();
        var ratioY = ratioX;

        var x0 = _layout.X0;
        var y0 = _layout.Y0;

        var dragOffset = (newGuiPosition - _guiStartPosition) / ratioX;

        var snapOffset = new Vector(double.PositiveInfinity, double.PositiveInfinity);

        var xAnchors = new List<Anchor>();
        var yAnchors = new List<Anchor>();

        /*var shift = */
        ShiftMonitor(dragOffset);

        //use anchors when control key is not pressed
        if (useAnchors)
        {
            foreach (var other in _layout.PhysicalMonitors.Where(m => m != _monitor))
            {
                foreach (var xAnchorThis in FrameMover.VerticalAnchors(_monitor, X - _dragStartPosition.X))
                {
                    foreach (var xAnchorOther in FrameMover.VerticalAnchors(other, 0))
                    {
                        var xOffset = xAnchorOther.Pos - xAnchorThis.Pos;

                        // if new offset is just equal to last, Add the new anchor visualization
                        if (Math.Abs(xOffset - snapOffset.X) < 0.01)
                        {
                            snapOffset = snapOffset.WithX(xOffset);
                            xAnchors.Add(xAnchorOther);
                            xAnchors.Add(new Anchor(xAnchorThis.Monitor, xAnchorOther.Pos, xAnchorThis.Brush, xAnchorThis.StrokeDashArray));
                        }
                        // if new offset is better than old one, Remove all visuals and Add the new one
                        else if ((Math.Abs(xOffset) < Math.Abs(snapOffset.X)))
                        {
                            snapOffset = snapOffset.WithX(xOffset);
                            xAnchors.Clear();
                            xAnchors.Add(xAnchorOther);
                            xAnchors.Add(new Anchor(xAnchorThis.Monitor, xAnchorOther.Pos, xAnchorThis.Brush, xAnchorThis.StrokeDashArray));
                        }
                    }
                }

                foreach (var yAnchorThis in HorizontalAnchors(_monitor, Y - _dragStartPosition.Y))
                {
                    foreach (var yAnchorOther in HorizontalAnchors(other, 0))
                    {
                        var yOffset = yAnchorOther.Pos - yAnchorThis.Pos;
                        // if new offset is just egual to last, Add the new anchor visualization
                        if (Math.Abs(yOffset - snapOffset.Y) < 0.01)
                        {
                            snapOffset = snapOffset.WithY(yOffset);
                            yAnchors.Add(yAnchorOther);
                            yAnchors.Add(new Anchor(yAnchorThis.Monitor, yAnchorOther.Pos, yAnchorThis.Brush, yAnchorThis.StrokeDashArray));
                        }
                        // if new offset is better than old one, Remove all visuals and Add the new one
                        else if ((Math.Abs(yOffset) < Math.Abs(snapOffset.Y)))
                        {
                            snapOffset = snapOffset.WithY(yOffset);
                            yAnchors.Clear();
                            yAnchors.Add(yAnchorOther);
                            yAnchors.Add(new Anchor(yAnchorThis.Monitor, yAnchorOther.Pos, yAnchorThis.Brush, yAnchorThis.StrokeDashArray));
                        }
                    }
                }
            }


            //Apply offset if under maximal snap distance
            if (Math.Abs(snapOffset.X) > maxSnapDistance)
            {
                xAnchors.Clear();
                snapOffset = snapOffset.WithX(0);
            }

            if (Math.Abs(snapOffset.Y) > maxSnapDistance)
            {
                yAnchors.Clear();
                snapOffset = snapOffset.WithY(0);
            }

            dragOffset += snapOffset;
        }

        /*shift = */
        ShiftMonitor(dragOffset);

        foreach (var anchor in xAnchors)
        {
            var t = ReferenceEquals(anchor.Monitor, _monitor) ? 5 : 2;
            var x = ratioX * (x0 + anchor.Pos);
            var l = ReferenceEquals(anchor.Monitor, _monitor)
                ? new Line
                {
                    StartPoint = new Point(x, _frame.Margin.Top),
                    EndPoint = new Point(x, _frame.Margin.Top + _frame.Bounds.Height),
                    Stroke = anchor.Brush,

                    Effect = new DropShadowEffect
                    {
                        Color = Colors.Transparent,
                        BlurRadius = 20,
                        Opacity = 1,
                        OffsetX = 0,
                        OffsetY = 0
                    },

                    StrokeDashArray = anchor.StrokeDashArray
                }
                : new Line
                {
                    StartPoint = new Point(x, ratioY * (y0 + anchor.Monitor.DepthProjection.OutsideY)),
                    EndPoint = new Point(x,ratioY * (y0 + anchor.Monitor.DepthProjection.OutsideBounds.Bottom)),
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

            var l = ReferenceEquals(anchor.Monitor, _monitor) ?
                new Line
                {
                    StartPoint = new Point(_frame.Margin.Left, y),
                    EndPoint = new Point(_frame.Margin.Left + _frame.Bounds.Width, y),
                    //this.FindParent<MultiScreensView>().BackgroundGrid.ActualWidth,
                    Stroke = anchor.Brush,
                    //StrokeThickness = 4,
                    //StrokeDashArray = new DoubleCollection { 5, 3 }

                    Effect = new DropShadowEffect
                    {
                        Color = Colors.Transparent,
                        BlurRadius = 20,
                        Opacity = 1,
                        OffsetX = 0,
                        OffsetY = 0
                    },
                    StrokeDashArray = anchor.StrokeDashArray
                } :
                new Line
                {
                    StartPoint = new Point(ratioX * (x0 + anchor.Monitor.DepthProjection.OutsideX), y),
                    EndPoint = new Point(ratioX * (x0 + anchor.Monitor.DepthProjection.OutsideBounds.Right), y),                     
                    Stroke = anchor.Brush,
                    StrokeDashArray = anchor.StrokeDashArray,
                    Opacity = 0.6,
                    Effect = new DropShadowEffect
                    {
                        Color = Colors.White,
                        BlurRadius = 20,
                        Opacity = 1,
                        OffsetX = 0,
                        OffsetY = 0
                    },
                };

            _anchorsCanvas.Children.Add(l);
        }
    }
}