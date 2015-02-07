using System.Diagnostics;

namespace MouseKeyboardActivityMonitor.WinApi
{
    /// <summary>
    /// Provides methods for subscription and unsubscription to global mouse and keyboard hooks.
    /// </summary>
    public class GlobalHooker : Hooker
    {
        internal override int Subscribe(int hookId, HookCallback hookCallback)
        {
            int hookHandle = HookNativeMethods.SetWindowsHookEx(
                hookId,
                hookCallback,
                Process.GetCurrentProcess().MainModule.BaseAddress,
                0);

            if (hookHandle == 0)
            {
                ThrowLastUnmanagedErrorAsException();
            }

            return hookHandle;
        }

        internal override bool IsGlobal
        {
            get { return true; }
        }

        /// <summary>
        /// Windows NT/2000/XP/Vista/7: Installs a hook procedure that monitors low-level mouse input events.
        /// </summary>
        internal const int WH_MOUSE_LL = 14;

        /// <summary>
        /// Windows NT/2000/XP/Vista/7: Installs a hook procedure that monitors low-level keyboard  input events.
        /// </summary>
        internal const int WH_KEYBOARD_LL = 13;
    }
}
