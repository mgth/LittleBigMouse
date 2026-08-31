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
/// <para>
/// Lifetime: the watcher owns one background thread and two OS wait handles (<see cref="_stop"/>
/// and the per-iteration notify event). <see cref="Dispose"/> is idempotent, signals the thread
/// to leave its wait, joins it so nothing touches <see cref="_stop"/> after it is released, then
/// disposes the stop event. The factory that owns it is a process-lifetime singleton, but a
/// layout rebuild is free to construct and dispose fresh watchers, so the handles must actually
/// come back — hence the join and the explicit dispose rather than leaning on the finalizer.
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
    readonly Func<WaitHandle, bool> _waitForChange;
    int _disposed;

    /// <summary>Production ctor: watches <c>HKCU\Control Panel\Desktop</c> via the registry.</summary>
    public WindowsWallpaperWatcher() : this(RegistryWaitForChange) { }

    /// <summary>
    /// Testable ctor. <paramref name="waitForChange"/> blocks until either a wallpaper change is
    /// observed (return <c>true</c> — the loop raises <see cref="Changed"/> and waits again) or the
    /// passed stop handle is signalled / the source can no longer be watched (return <c>false</c> —
    /// the loop exits). This is the exact seam the registry implementation sits behind, so the
    /// start/stop/dispose lifetime is provable off Windows without touching HKCU.
    /// </summary>
    internal WindowsWallpaperWatcher(Func<WaitHandle, bool> waitForChange)
    {
        _waitForChange = waitForChange;
        _thread = new Thread(Watch) { IsBackground = true, Name = "LbmWallpaperWatcher" };
        _thread.Start();
    }

    void Watch()
    {
        try
        {
            while (!_stop.IsSet)
            {
                // Blocks until a change is seen (true) or we are asked to stop / the source is gone
                // (false). The stop handle is passed in so the wait can abandon on Dispose().
                if (!_waitForChange(_stop.WaitHandle)) return;
                if (_stop.IsSet) return;

                Changed?.Invoke(this, EventArgs.Empty);
            }
        }
        catch
        {
            // The watcher thread must never take the app down.
        }
    }

    /// <summary>The real Win32 wait: register an async notification and block until it or the stop fires.</summary>
    static bool RegistryWaitForChange(WaitHandle stop)
    {
        // Opened read-only, which still grants KEY_NOTIFY. Re-opened per call so the handle never
        // outlives a single wait — the key can be reopened cheaply and this keeps the seam simple.
        using var key = Registry.CurrentUser.OpenSubKey(@"Control Panel\Desktop");
        if (key is null) return false;

        using var notify = new ManualResetEvent(false);

        // Asynchronous: returns immediately and signals `notify` when a value under the Desktop key
        // (WallPaper, WallpaperStyle, TranscodedImageCache, per-monitor...) changes.
        if (RegNotifyChangeKeyValue(key.Handle, true,
                REG_NOTIFY_CHANGE_NAME | REG_NOTIFY_CHANGE_LAST_SET,
                notify.SafeWaitHandle, true) != 0)
            return false; // registration failed — give up rather than spin

        // 0 == a change; 1 == Dispose() requested.
        return WaitHandle.WaitAny(new[] { stop, notify }) == 1;
    }

    public void Dispose()
    {
        // Idempotent: only the first caller runs the teardown. A second call — or a call racing the
        // first — returns without touching the already-released handle.
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;

        _stop.Set();

        // Join before disposing _stop: the watcher thread reads _stop.WaitHandle / _stop.IsSet, so
        // releasing the event out from under it would be an ObjectDisposedException race.
        if (_thread.IsAlive && _thread != Thread.CurrentThread)
            _thread.Join(TimeSpan.FromSeconds(2));

        _stop.Dispose();
    }

    [DllImport("advapi32.dll", SetLastError = true)]
    static extern int RegNotifyChangeKeyValue(
        SafeRegistryHandle hKey, bool bWatchSubtree, uint dwNotifyFilter, SafeWaitHandle hEvent, bool fAsynchronous);
}
