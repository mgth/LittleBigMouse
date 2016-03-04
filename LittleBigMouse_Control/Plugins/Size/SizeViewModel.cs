using System;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Effects;
using System.Windows.Shapes;
using LittleBigMouse_Control.SizePlugin;

namespace LittleBigMouse_Control.SizePlugin
{
    class CotationMark
    {
        public void SetPoints(double length, double x1, double y1, double x2, double y2)
        {
            bool result = SetArrowLength(length);
            result |= SetStartPoint(new Point(x1, y1));
            result |= SetEndPoint(new Point(x2, y2));

            if (result) SetArrows();                
        }
        public double ArrowLength
        {
            get { return _arrowLength; }
            set
            {
                if (SetArrowLength(value))
                SetArrows();
            }
        }

        private bool SetArrowLength(double length)
        {
            if (_arrowLength == length) return false;
            _arrowLength = length;
            return true;
        }

        public Point StartPoint
        {
            get { return _startPoint; }
            set
            {
                if (SetStartPoint(value))
                SetArrows();            
            }
        }

        private bool SetStartPoint(Point p)
        {
            if (_startPoint == p) return false;
                _startPoint = p;
                _line.X1 = _startPoint.X;
                _line.Y1 = _startPoint.Y;
            return true;
        }

        private void SetArrows()
        {
            SetArrow(_startArrow,StartPoint,EndPoint,ArrowLength);
            SetArrow(_endArrow, EndPoint, StartPoint, ArrowLength);
        }

        private void SetArrow(Polygon p, Point start, Point end, double length  = 1)
        {
            p.Points[0] = start;

            Vector v = end - start;

            v.Normalize();
            v *= length;

            Vector v1 = new Vector(v.Y/2,-v.X/2);
            Vector v2 = new Vector(-v.Y/2,v.X/2);

            p.Points[1] = start + v + v1;
            p.Points[2] = start + v + v2;
        }

        public Point EndPoint
        {
            get { return _endPoint; }
            set
            {
                SetStartPoint(value);
                SetArrows();
            }
        }

        private bool SetEndPoint(Point p)
        {
            if (_endPoint == p) return false;
                _endPoint = p;
                _line.X2 = _endPoint.X;
                _line.Y2 = _endPoint.Y;
            return true;
        }

        private Brush _brush;

        public Brush Brush
        {
            get
            {
                return _brush;
            }
            set
            {
                _brush = value;
                _line.Stroke = _brush;
                _startArrow.Fill = _brush;
                _endArrow.Fill = _brush;
            }
        }

        public void AddTo(Canvas c)
        {
            c.Children.Add(_line);
            c.Children.Add(_startArrow);
            c.Children.Add(_endArrow);
        }

        private readonly Line _line = new Line { StrokeThickness = 2};
        private readonly Polygon _startArrow = new Polygon {Points = {new Point(), new Point(), new Point(), } };
        private readonly Polygon _endArrow = new Polygon { Points = { new Point(), new Point(), new Point(), } };

        private Point _startPoint;
        private Point _endPoint;
        private double _arrowLength;
    }
    class SizeViewModel : ScreenControlViewModel
    {
        public override Type ViewType => typeof(Plugins.Size.SizeScreenView);

        public SizeViewModel()
        {
            _canvas = new Canvas { Effect = _effect };

            _ousideHorizontalCotation.AddTo(_canvas);
            _outsideVerticalCotation.AddTo(_canvas);
            _insideHorizontalCotation.AddTo(_canvas);
            _insideVerticalCotation.AddTo(_canvas);

            InsideCoverControl.Children.Add(_canvas);

            InsideCoverControl.LayoutUpdated += OnFrameSizeChanged;            
        }

        private readonly CotationMark _ousideHorizontalCotation = new CotationMark {Brush = new SolidColorBrush(Colors.CadetBlue)};
        private readonly CotationMark _outsideVerticalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.CadetBlue) };
        private readonly CotationMark _insideHorizontalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.Bisque) };
        private readonly CotationMark _insideVerticalCotation = new CotationMark { Brush = new SolidColorBrush(Colors.Bisque) };


        private void OnFrameSizeChanged(object sender, EventArgs eventArgs)
        {
            DrawLines();
        }

        public Grid InsideCoverControl { get; } = new Grid();
        readonly Effect _effect = new DropShadowEffect
        {
            Color = Colors.DarkBlue,
        };

        private readonly Canvas _canvas;

         public void DrawLines()
        {
            double r = Frame.Presenter.Ratio;

            double h = InsideCoverControl.ActualHeight;
            double w = InsideCoverControl.ActualWidth;
            double x = 5 * InsideCoverControl.ActualWidth / 8;// + ScreenGui.LeftBorder.Value;
            double y = 5 * InsideCoverControl.ActualHeight / 8;// + ScreenGui.TopBorder.Value;

            double x2 = - r * Screen.LeftBorder;
            double y2 = - r * Screen.TopBorder;

            double h2 = h - y2 + r * Screen.BottomBorder;
            double w2 = w - x2 + r * Screen.RightBorder;

            double arrow = r*(Screen.BottomBorder + Screen.RightBorder + Screen.LeftBorder + Screen.TopBorder)/8;

            _insideVerticalCotation.SetPoints(arrow, x, 0,x,h);
            _insideHorizontalCotation.SetPoints (arrow, 0, y, w, y);

            _outsideVerticalCotation.SetPoints(arrow, x + (w/8)-(w/128),y2,x + (w/8)-(w/128),y2+h2);
            _ousideHorizontalCotation.SetPoints(arrow, x2, y + (h/8)-(h/128),x2+w2,y + (h/8)-(h/128));
        }
    }
}
