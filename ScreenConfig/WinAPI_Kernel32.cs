using System.Runtime.InteropServices;

namespace WinAPI_Kernel32
{
    class Kernel32
    {
        internal const uint ERROR_NO_MORE_ITEMS = 259;

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();
    }
}
