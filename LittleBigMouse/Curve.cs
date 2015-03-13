using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Windows.Controls;
using System.Runtime.InteropServices;
using System.Windows.Media;
using System.Windows;
using System.ComponentModel;
using System.Windows.Threading;
using System.Globalization;

namespace LittleBigMouse
{
    public class Curve : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private void changed(String name)
        {
            if (PropertyChanged != null) PropertyChanged(this, new PropertyChangedEventArgs(name));
        }

        public event EventHandler CurveChanged;
        private void OnCurveChanged()
        {
            if (CurveChanged != null) CurveChanged(this, new EventArgs());
        }


        private double[] _values;
        Color _color;
        public Color Color { get { return _color; } }

        public Curve() { }

        public Curve(int size, Color color)
        {
            _color = color;
            _values = new double[size];
        }

        public Curve(UInt16[] values, Color color)
        {
            _color = color;
            _values = new double[values.Length];
            SetValues(values);
        }

        public void SetValues(UInt16[] values)
        {
            for (int i = 0; i < Math.Min(values.Length, _values.Length); i++) _values[i] = (double)values[i];
            OnCurveChanged();
        }
        public void SetValues(double[] values)
        {
            for (int i = 0; i < Math.Min(values.Length, _values.Length); i++) _values[i] = (double)values[i];
            OnCurveChanged();
        }
        public void SetValue(uint pos, double value)
        {
            if (_values.Length > pos)
            {
                _values[pos] = value;
                OnCurveChanged();
            }
        }

        public double GetValue(uint pos)
        {
            if (pos < _values.Length)
                return _values[pos];
            else
                return 0;
        }

        public double this[uint key]
        {
            get
            {
                return GetValue(key);
            }
            set
            {
                SetValue(key, value);
            }
        }

        public int Length { get { return _values.Length; } }
        public double MaxHeight
        {
            get
            {
                double max = 0;
                for (int i = 0; i < _values.Length; i++)
                    if (_values[i] > max) max = _values[i];
                return max;
            }
        }

    }

    class CurveViewer : Grid
    {
        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Ansi)]
        public struct RAMP
        {
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 256)]
            public UInt16[] Red;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 256)]
            public UInt16[] Green;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 256)]
            public UInt16[] Blue;
        }

        [DllImport("gdi32.dll")]
        public static extern int GetDeviceGammaRamp(IntPtr hDC, ref RAMP lpRamp);

        [DllImport("gdi32.dll")]
        public static extern int SetDeviceGammaRamp(IntPtr hDC, ref RAMP lpRamp);

        [DllImport("gdi32.dll")]
        static extern IntPtr CreateDC(string lpszDriver, string lpszDevice, string lpszOutput, IntPtr lpInitData);

        private IntPtr _hdcDevice = IntPtr.Zero;

        private List<Curve> _curves = new List<Curve>();

        public void LoadLut(string DeviceName)
        {
            IntPtr hdcDevice = CreateDC(DeviceName, "", "", IntPtr.Zero);

            RAMP TableLut = new RAMP();
            GetDeviceGammaRamp(hdcDevice, ref TableLut);

            _curves.Add(new Curve(TableLut.Red, Colors.Red));
            _curves.Add(new Curve(TableLut.Green, Colors.Red));
            _curves.Add(new Curve(TableLut.Blue, Colors.Red));

            InvalidateVisual();
            if (OnPositionChange != null)
                OnPositionChange(this, new EventArgs());
        }

        public void AddCurve(Curve curve)
        {
            _curves.Add(curve);

            curve.CurveChanged += Curve_CurveChanged;

            InvalidateVisual();
            if (OnPositionChange != null)
                OnPositionChange(this, new EventArgs());
        }

        private void Curve_CurveChanged(object sender, EventArgs e)
        {
            Application.Current.Dispatcher.BeginInvoke(DispatcherPriority.Normal, new Action(() =>
            {
                InvalidateVisual();
                if (OnPositionChange != null)
                    OnPositionChange(this, new EventArgs());
            }));
        }


        public void Clear()
        {
            _curves.Clear();

            InvalidateVisual();
            if (OnPositionChange != null)
                OnPositionChange(this, new EventArgs());
        }

        private int _position = -1;
        public int Position
        {
            get
            {
                return _position;
            }
            set
            {
                if (value <= this.MaxCurveLength)
                {
                    _position = value;
                    InvalidateVisual();
                    if (OnPositionChange != null)
                        OnPositionChange(this, new EventArgs());
                }
            }
        }

        public event EventHandler OnPositionChange;

        public CurveViewer()
        {
        }

        private int MaxCurveLength
        {
            get
            {
                int max = 0;
                foreach (Curve c in _curves)
                    if (c.Length > max) max = c.Length;

                return max;
            }
        }
        private double MaxCurveHeight
        {
            get
            {
                double max = 0;
                foreach (Curve c in _curves)
                {
                    double m = c.MaxHeight;
                    if (m > max) max = m;
                }

                return max;
            }
        }

        protected override void OnRender(DrawingContext dc)
        {
            dc.DrawRectangle(Brushes.Black, null, new Rect(0, 0, ActualWidth, ActualHeight));

            //if( System.ComponentModel.DesignerProperties..IsInDesignMode
            if (!System.ComponentModel.DesignerProperties.GetIsInDesignMode(new DependencyObject()))//Application.Current.MainWindow) )
            {
                double ratioY = ActualHeight / MaxCurveHeight;
                double ratioX = ActualWidth / MaxCurveLength;

                {
                    dc.DrawLine(new Pen(Brushes.DimGray, 1.0), new Point(_position * ratioX + 0.5, 0.0), new Point(_position * ratioX + 0.5, ActualHeight));
                }

                foreach (Curve c in _curves)
                {
                    Pen p = new Pen(new SolidColorBrush(c.Color), 1.0);
                    if (_position >= 0 && _position <= MaxCurveLength)
                    {
                        FormattedText formattedText = new FormattedText(_position.ToString() + ":" + c[(uint)_position].ToString(), CultureInfo.CurrentCulture,
                                          FlowDirection.LeftToRight,
                                          new Typeface(new FontFamily("Arial").ToString()),
                                          6, new SolidColorBrush(c.Color));

                        dc.DrawText(
                            formattedText, 
                            new Point(_position*ratioX + 0.5, ActualHeight - c[(uint)_position]*ratioY + 0.5)
                            );
                    }

                    Point a = new Point(0.5, ActualHeight - (double)c[0] * ratioY - 0.5);

                    Point n;

                    for (uint i = 1; i < c.Length; i++)
                    {
                        n = new Point((double)i * ratioX + 0.5, ActualHeight - (double)c[i] * ratioY - 0.5);
                        dc.DrawLine(p, a, n);
                        a = n;
                    }
                }
            }

            base.OnRender(dc);
        }

        protected override void OnMouseDown(System.Windows.Input.MouseButtonEventArgs e)
        {
            Position = (int)(MaxCurveLength * e.GetPosition(this).X / ActualWidth);
            base.OnMouseDown(e);
        }

        protected override void OnMouseMove(System.Windows.Input.MouseEventArgs e)
        {
            if (e.LeftButton == System.Windows.Input.MouseButtonState.Pressed)
                Position = (int)(MaxCurveLength * e.GetPosition(this).X/ActualWidth);

            base.OnMouseMove(e);
        }
    }
}

