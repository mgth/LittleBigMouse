using System.Windows.Media;
using LittleBigMouse.ScreenConfig;

namespace LittleBigMouse.Control.Core
{
    public class Anchor
    {
        public Screen Screen { get; }
        public double Pos { get; }
        public Brush Brush { get; }
        public DoubleCollection StrokeDashArray { get; }
        public Anchor(Screen screen, double pos, Brush brush, DoubleCollection strokeDashArray )
        {
            Screen = screen;
            Pos = pos;
            Brush = brush;
            StrokeDashArray = strokeDashArray;
        }
    }
}