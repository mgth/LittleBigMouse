using System;
using System.Runtime.InteropServices;
using System.Text;

namespace WinAPI_AdvAPI32
{
    class AdvAPI32
    {
        [DllImport("advapi32.dll", SetLastError = true, CharSet =CharSet.Unicode)]
        internal static extern uint RegEnumValue(
              IntPtr hKey,
              uint dwIndex,
              StringBuilder lpValueName,
              ref uint lpcValueName,
              IntPtr lpReserved,
              ref UInt32 lpType,
              IntPtr lpData,
              ref UInt32 lpcbData);

        [DllImport("advapi32.dll", SetLastError = true)]
        internal static extern int RegCloseKey(
            IntPtr hKey);
    }
}
