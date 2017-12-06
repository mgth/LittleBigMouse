using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using WindowsMonitors.Annotations;
using Erp.Notify;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayMode
    {
        public Point Position { get; }
        public int BitsPerPixel { get; }
        public Size Pels { get; }
        public int DisplayFlags { get; }
        public int DisplayFrequency { get; }
        public int DisplayFixedOutput { get; }
        public int DisplayOrientation { get; }
        public DisplayMode(NativeMethods.DEVMODE devmode)
        {
            DisplayOrientation = ((devmode.Fields & NativeMethods.DM.DisplayOrientation) != 0)?devmode.DisplayOrientation:0;
            Position = ((devmode.Fields & NativeMethods.DM.Position) != 0)?new Point(devmode.Position.x, devmode.Position.y):new Point(0,0);
            BitsPerPixel = ((devmode.Fields & NativeMethods.DM.BitsPerPixel) != 0)?devmode.BitsPerPel:0;
            Pels = ((devmode.Fields & (NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight)) != 0)? new Size(devmode.PelsWidth, devmode.PelsHeight):new Size(1,1);
            DisplayFlags = ((devmode.Fields & NativeMethods.DM.DisplayFlags) != 0)? devmode.DisplayFlags:0;
            DisplayFrequency = ((devmode.Fields & NativeMethods.DM.DisplayFrequency) != 0)? devmode.DisplayFrequency:0;
            DisplayFixedOutput = ((devmode.Fields & NativeMethods.DM.DisplayFixedOutput) != 0)? devmode.DisplayFixedOutput:0;
        }
    }
}
