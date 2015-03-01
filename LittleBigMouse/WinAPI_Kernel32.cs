using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;

namespace WinAPI_Kernel32
{
    class Kernel32
    {
        internal const uint ERROR_NO_MORE_ITEMS = 259;

        [DllImport("kernel32.dll")]
        public static extern uint GetLastError();
    }
}
