using System.Windows;

namespace HLab.Sys.MouseHooker
{
    public class HookMouseEventArg
    {
        public HookMouseEventArg(Point point)
        {
            Point = point;
        }

        public Point Point { get; set; }
        public bool Handled { get; set; } = false;
    }
}