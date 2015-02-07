using System;
using System.Runtime.InteropServices;

namespace MouseKeyboardActivityMonitor.WinApi
{
    /// <summary>
    /// Provides methods for subscription and unsubscription to application mouse and keyboard hooks.
    /// </summary>
    public class AppHooker : Hooker
    {
        /// <summary>
        /// Installs a hook procedure that monitors mouse messages. For more information, see the MouseProc hook procedure. 
        /// </summary>
        internal const int WH_MOUSE = 7;

        /// <summary>
        /// Installs a hook procedure that monitors keystroke messages. For more information, see the KeyboardProc hook procedure. 
        /// </summary>
        internal const int WH_KEYBOARD = 2;

        internal override int Subscribe(int hookId, HookCallback hookCallback)
        {
            int hookHandle = HookNativeMethods.SetWindowsHookEx(
                hookId,
                hookCallback,
                IntPtr.Zero,
                ThreadNativeMethods.GetCurrentThreadId());

            if (hookHandle == 0)
            {
                ThrowLastUnmanagedErrorAsException();
            }

            return hookHandle;
        }

        internal override bool IsGlobal
        {
            get { return false; }
        }
    }
}