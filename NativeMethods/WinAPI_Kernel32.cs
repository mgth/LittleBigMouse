using System.Runtime.InteropServices;

namespace WinAPI
{
    [System.Security.SuppressUnmanagedCodeSecurity]
    public static partial class NativeMethods
    {
        public const uint ERROR_NO_MORE_ITEMS = 259;

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();
    }
}
