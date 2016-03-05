using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using WinAPI;

namespace LbmScreenConfig
{
    public static class WindowExt
    {
        public static void EnableBlur(this Window win)
        {
            WindowInteropHelper windowHelper = new WindowInteropHelper(win);

            var accent = new NativeMethods.AccentPolicy();
            var accentStructSize = Marshal.SizeOf(accent);
            accent.AccentState = NativeMethods.AccentState.ACCENT_ENABLE_BLURBEHIND;

            var accentPtr = Marshal.AllocHGlobal(accentStructSize);
            Marshal.StructureToPtr(accent, accentPtr, false);

            var data = new NativeMethods.WindowCompositionAttributeData();
            data.Attribute = NativeMethods.WindowCompositionAttribute.WCA_ACCENT_POLICY;
            data.SizeOfData = accentStructSize;
            data.Data = accentPtr;

            NativeMethods.SetWindowCompositionAttribute(windowHelper.Handle, ref data);

            Marshal.FreeHGlobal(accentPtr);
        }
    }
}
