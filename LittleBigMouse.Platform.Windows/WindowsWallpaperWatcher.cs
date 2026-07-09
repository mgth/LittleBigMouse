#nullable enable
using System;
using System.Runtime.InteropServices;
using System.Threading;
using Microsoft.Win32;
using Microsoft.Win32.SafeHandles;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Raises <see cref="Changed"/> when the Windows desktop wallpaper settings change, using
/// <c>RegNotifyChangeKeyValue</c> on <c>HKCU\Control Panel\Desktop</c>. A dedicated background
/// thread blocks on the change event (no polling, no CPU while idle).
/// <para>
/// This is the reliable detector where the alternatives are not: the managed
/// <c>SystemEvents.UserPreferenceChanged</c> never fires in this process, and the daemon's
/// <c>WM_SETTINGCHANGE</c> broadcast is dropped intermittently (UIPI filtering + a message pump
/// shared with the mouse hook). Watching the registry key the wallpaper is written to sidesteps
/// all of that.
/// </para>
/// </summary>
public sealed class WindowsWallpaperWatcher : IDisposable
{
    const uint REG_NOTIFY_CHANGE_NAME = 0x00000001;
    const uint REG_NOTIFY_CHANGE_LAST_SET = 0x00000004;

    /// <summary>Raised on the watcher's background thread whenever a Desktop registry value changes.</summary>
    public event EventHandler? Changed;

    readonly Thread _thread;
    readonly ManualResetEventSlim _stop = new(false);

    public WindowsWallpaperWatcher()
    {
        _thread = new Thread(Watch) { IsBackground = true, Name = "LbmWallpaperWatcher" };
        _thread.Start();
    }

    void Watch()
    {
        try
        {
            // Opened read-only, which still grants KEY_NOTIFY. Kept open for the watcher's lifetime.
            using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop");
            if (key is null) return;

            using var notify = new ManualResetEvent(false);
            var waits = new[] { _stop.WaitHandle, notify };

            while (!_stop.IsSet)
            {
                notify.Reset();

                // Asynchronous: returns immediately and signals `notify` when a value under the
                // Desktop key (WallPaper, WallpaperStyle, TranscodedImageCache, per-monitor...) changes.
                if (RegNotifyChangeKeyValue(key.Handle, true,
                        REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                        notify.SafeWaitHandle, true) != 0)
                    return; // registration failed — give up rather than spin

                if (WaitHandle.WaitAny(waits) == 0) return; // Dispose() requested

                Changed?.Invoke(this, EventArgs.Empty);
            }
        }
        catch
        {
            // The watcher thread must never take the app down.
        }
    }

    public void Dispose() => _stop.Set();

    [DllImport("advapi32.dll", SetLastError = true)]
    static extern int RegNotifyChangeKeyValue(
        SafeRegistryHandle hKey, bool bWatchSubtree, uint dwNotifyFilter, SafeWaitHandle hEvent, bool fAsynchronous);
}
