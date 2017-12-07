/*
  HLab.Windows.API
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.API.

    HLab.Windows.API is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.API is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Runtime.InteropServices;
using System.Security;
// ReSharper disable InconsistentNaming

namespace WinAPI
{
    [SuppressUnmanagedCodeSecurity]
    public static partial class NativeMethods
    {
        public enum KEY_INFORMATION_CLASS
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
            public uint NameLength;     // The size, in bytes, of the key name string in the Name array.
            public char[] Name;           // An array of wide characters that contains the name of the key.
                                          // This character string is not null-terminated.
                                          // Only the first element in this array is included in the
                                          //    KEY_NAME_INFORMATION structure definition.
                                          //    The storage for the remaining elements in the array immediately
                                          //    follows this element.
        }
        [DllImport("ntdll.dll", SetLastError = true, CharSet = CharSet.Unicode)]
        public static extern uint ZwQueryKey(IntPtr hKey, KEY_INFORMATION_CLASS KeyInformationClass, IntPtr lpKeyInformation, int Length, out int ResultLength);
    }
}
