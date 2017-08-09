using System;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Shapes;
using Erp.Notify;
using LbmScreenConfig;
using LittleBigMouse_Control.PluginLocation;

namespace LittleBigMouse_Control.Plugins.Location
{
    class LocationScreenViewModel : ScreenControlViewModel
    {
        public override Type ViewType => typeof (Plugins.Location.LocationScreenView);

        public LocationPlugin Plugin
        {
            get => this.Get<LocationPlugin>(); set => this.Set(value);
        }

        [TriggedOn("Screen.PhysicalRatioX")]
        public double RatioX
        {
            get => Screen.PhysicalRatioX * 100; set { Screen.PhysicalRatioX = value/100; Screen.Config.Compact(); }
        }

        [TriggedOn("Screen.PhysicalRatioY")]
        public double RatioY
        {
            get => Screen.PhysicalRatioY * 100; set { Screen.PhysicalRatioY = value / 100; Screen.Config.Compact(); }
        }

        [TriggedOn("Screen.Orientation")]
        public VerticalAlignment DpiVerticalAlignment
            => (Screen.Monitor.DisplayOrientation == 3) ? VerticalAlignment.Bottom : VerticalAlignment.Top;

        [TriggedOn("Screen.Orientation")]
        public VerticalAlignment PnpNameVerticalAlignment
            => (Screen.Monitor.DisplayOrientation == 2) ? VerticalAlignment.Bottom : VerticalAlignment.Top;


        private Point _guiStartPosition;
        private Point _guiLastPosition;
        private Point _dragStartPosition;
        private Point _dragLastPosition;

        private Vector ShiftScreen(Vector offset)
        {
            Point pos = _dragStartPosition + offset;
            Screen.LocationInMm = pos;
            Vector shift = Screen.LocationInMm - pos;
            Screen.Config.ShiftMovingPhysicalBounds(shift);
            _dragStartPosition += shift;
            return shift;
        }

        public void StartMove(Point p)
        {
            _guiStartPosition = p;
            _guiLastPosition = _guiStartPosition;
            _dragStartPosition = Screen.LocationInMm;
            _dragLastPosition = _dragStartPosition;

            Screen.Config.Moving = true;
            Screen.Selected = true;
        }

        public void EndMove()
        {
            if (!Screen.Config.Moving) return;

            Plugin.VerticalAnchors.Children.Clear();
            Plugin.HorizontalAnchors.Children.Clear();

            Screen.Config.Moving = false;
            Screen.Config.UpdatePhysicalOutsideBounds();

            Screen.Config.Compact();

            //Todo : Plugin.ActivateConfig();
        }

        public void Move(Point newGuiPosition)
        {
            const double maxSnapDistance = 10.0;

            if (!Screen.Config.Moving) return;

            Vector dragOffset = (newGuiPosition - _guiStartPosition) / Frame.Presenter.Ratio;

            Vector snapOffset = new Vector(double.PositiveInfinity, double.PositiveInfinity);

            List<Anchor> xAnchors = new List<Anchor>();
            List<Anchor> yAnchors = new List<Anchor>();

            Vector shift = ShiftScreen(dragOffset);

            //use anchors when control key is not pressed
            if ((Keyboard.Modifiers & ModifierKeys.Control) == 0)
            {
                foreach (Screen s in Screen.OtherScreens)
                {
                    foreach (Anchor xAnchorThis in VerticalAnchors(Screen))
                    {
                        foreach (Anchor xAnchorOther in VerticalAnchors(s))
                        {
                            double xOffset = xAnchorOther.Pos - xAnchorThis.Pos;

                            // if new offset is just egual to last, Add the new anchor visualization
                            if (Math.Abs(xOffset - snapOffset.X) < 0.01)
                            {
                                snapOffset.X = xOffset;
                                xAnchors.Add(xAnchorOther);
                            }
                            // if new offset is better than old one, Remove all visuals and Add the new one
                            else if ((Math.Abs(xOffset) < Math.Abs(snapOffset.X)))
                            {
                                snapOffset.X = xOffset;
                                xAnchors.Clear();
                                xAnchors.Add(xAnchorOther);
                            }
                        }
                    }

                    foreach (Anchor yAnchorThis in HorizontalAnchors(Screen))
                    {
                        foreach (Anchor yAnchorOther in HorizontalAnchors(s))
                        {
                            double yOffset = yAnchorOther.Pos - yAnchorThis.Pos;
                            // if new offset is just egual to last, Add the new anchor visualization
                            if (Math.Abs(yOffset - snapOffset.Y) < 0.01)
                            {
                                snapOffset.Y = yOffset;
                                yAnchors.Add(yAnchorOther);
                            }
                            // if new offset is better than old one, Remove all visuals and Add the new one
                            else if ((Math.Abs(yOffset) < Math.Abs(snapOffset.Y)))
                            {
                                snapOffset.Y = yOffset;
                                yAnchors.Clear();
                                yAnchors.Add(yAnchorOther);
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

            shift = ShiftScreen(dragOffset);

            Plugin.VerticalAnchors.Children.Clear();
            foreach (Anchor anchor in xAnchors)
            {
                double guiX = Frame.Presenter.PhysicalToUiX(anchor.Pos + shift.X);
                Line l = new Line()
                {
                    X1 = guiX,
                    X2 = guiX,
                    Y1 = 0,
                    Y2 = Frame.Presenter.ScreensCanvas.ActualHeight,
                    Stroke = anchor.Brush,
                    StrokeDashArray = new DoubleCollection { 5, 3 }
                };
                Plugin.VerticalAnchors.Children.Add(l);
            }

            Plugin.HorizontalAnchors.Children.Clear();
            foreach (Anchor anchor in yAnchors)
            {
                double guiY = Frame.Presenter.PhysicalToUiY(anchor.Pos + shift.Y);
                Line l = new Line()
                {
                    Y1 = guiY,
                    Y2 = guiY,
                    X1 = 0,
                    X2 = Frame.Presenter.ScreensCanvas.ActualWidth,
                    Stroke = anchor.Brush,
                    StrokeDashArray = new DoubleCollection { 5, 3 }
                };
                Plugin.HorizontalAnchors.Children.Add(l);
            }
        }
        public List<Anchor> VerticalAnchors(Screen s) => new List<Anchor>
                {
                     new Anchor(s,s.OutsideBoundsInMm.X,new SolidColorBrush(Colors.Chartreuse)),
                     new Anchor(s,s.XLocationInMm,new SolidColorBrush(Colors.LightGreen)),
                     new Anchor(s,s.XLocationInMm + s.WidthInMm /2,new SolidColorBrush(Colors.Red)),
                     new Anchor(s,s.BoundsInMm.Right,new SolidColorBrush(Colors.LightGreen)),
                     new Anchor(s,s.OutsideBoundsInMm.Right,new SolidColorBrush(Colors.Chartreuse)),
                };

        public List<Anchor> HorizontalAnchors(Screen s) => new List<Anchor>
                {
                     new Anchor(s,s.OutsideBoundsInMm.Y,new SolidColorBrush(Colors.Chartreuse)),
                     new Anchor(s,s.YLocationInMm,new SolidColorBrush(Colors.LightGreen)),
                     new Anchor(s,s.YLocationInMm + s.HeightInMm /2,new SolidColorBrush(Colors.Red)),
                     new Anchor(s,s.BoundsInMm.Bottom,new SolidColorBrush(Colors.LightGreen)),
                     new Anchor(s,s.OutsideBoundsInMm.Bottom,new SolidColorBrush(Colors.Chartreuse)),
                };


    }
}
