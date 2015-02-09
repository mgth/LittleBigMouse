using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace MouseControl
{
    public class Zone
    {
        private Screen _screen;
    }

    public class ZoneManual : Zone
    {
        public Rect Input;
        public Rect Output;
        public Screen Screen;
        public double Speed = 10;
        public int Size = 1;
        public ZoneManual(Screen screen, Rect input, Rect output, double speed=10, int size=1)
        {
            Screen = screen;
            Input = input;
            Output = output;
            Speed = speed;
            Size = size;
        }

        public bool Contains(Point p)
        {
            return Input.Contains(p);
        }

        public Point Translate(Point p)
        {
            double x = ((((p.X - Input.X) / Input.Width) * Output.Width) + Output.X);
            double y = ((((p.Y - Input.Y) / Input.Height) * Output.Height) + Output.Y);

            return new Point((int)x, (int)y);
        }
    }

    public class ZoneAuto : Zone
    {

    }

    public class ZoneH : ZoneAuto
    {
        protected double _top;
        protected double _bottom;
        protected double _width;
        protected double _offset;
    }

    public class ZoneRight : ZoneH
    {
        public ZoneRight(Screen screen, double top, double bottom,double width,double offset)
        {

        }
    }
}
