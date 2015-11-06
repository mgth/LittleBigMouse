using System;
using System.Runtime.InteropServices;

namespace WinAPI_Ntdll
{
    enum KEY_INFORMATION_CLASS
    {
        KeyBasicInformation,            // A KEY_BASIC_INFORMATION structure is supplied.
        KeyNodeInformation,             // A KEY_NODE_INFORMATION structure is supplied.
        KeyFullInformation,             // A KEY_FULL_INFORMATION structure is supplied.
        KeyNameInformation,             // A KEY_NAME_INFORMATION structure is supplied.
        KeyCachedInformation,           // A KEY_CACHED_INFORMATION structure is supplied.
        KeyFlagsInformation,            // Reserved for system use.
        KeyVirtualizationInformation,   // A KEY_VIRTUALIZATION_INFORMATION structure is supplied.
        KeyHandleTagsInformation,       // Reserved for system use.
        MaxKeyInfoClass                 // The maximum value in this enumeration type.
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct KEY_NAME_INFORMATION
    {
        public UInt32 NameLength;     // The size, in bytes, of the key name string in the Name array.
        public char[] Name;           // An array of wide characters that contains the name of the key.
                                      // This character string is not null-terminated.
                                      // Only the first element in this array is included in the
                                      //    KEY_NAME_INFORMATION structure definition.
                                      //    The storage for the remaining elements in the array immediately
                                      //    follows this element.
    }
    class Ntdll
    {
        [DllImport("ntdll.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern uint ZwQueryKey(IntPtr hKey, KEY_INFORMATION_CLASS KeyInformationClass, IntPtr lpKeyInformation, int Length, out int ResultLength);
    }
}
