using System.Windows;

namespace HLab.Sys.MouseHooker
{
    public class HookMouseEventArg
    {
        public Point Point { get; set; }
        public bool Handled { get; set; }
    }
}