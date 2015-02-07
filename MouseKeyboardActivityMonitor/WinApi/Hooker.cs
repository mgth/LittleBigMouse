using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

namespace MouseKeyboardActivityMonitor.WinApi
{
    /// <summary>
    /// Provides base implementation of methods for subscription and unsubscription to application and/or global mouse and keyboard hooks.
    /// </summary>
    public abstract class Hooker
    {
        internal abstract int Subscribe(int hookId, HookCallback hookCallback);

        internal void Unsubscribe(int handle)
        {
            int result = HookNativeMethods.UnhookWindowsHookEx(handle);

            // IFREQ: currently taken out as throws an exception at the very end
//            if (result == 0)
//            {
//                ThrowLastUnmanagedErrorAsException();
//            }
        }

        internal abstract bool IsGlobal { get; }

        internal static void ThrowLastUnmanagedErrorAsException()
        {
            //Returns the error code returned by the last unmanaged function called using platform invoke that has the DllImportAttribute.SetLastError flag set. 
            int errorCode = Marshal.GetLastWin32Error();
            //Initializes and throws a new instance of the Win32Exception class with the specified error. 
            throw new Win32Exception(errorCode);
        }
    }
}