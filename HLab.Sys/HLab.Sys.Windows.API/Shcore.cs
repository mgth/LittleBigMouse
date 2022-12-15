using System;
using System.Runtime.InteropServices;

namespace HLab.Sys.Windows.API;

public static partial class Shcore
{
    public enum DpiType
    {
        Effective = 0,
        Angular = 1,
        Raw = 2,
    }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

    [LibraryImport("Shcore.dll")]
    public static partial IntPtr GetDpiForMonitor(in IntPtr hmonitor, in DpiType dpiType, out uint dpiX, out uint dpiY);

    [LibraryImport("Shcore.dll")]
    public static partial int GetScaleFactorForMonitor(
        IntPtr hMonitor,
        ref int scale
    );
}