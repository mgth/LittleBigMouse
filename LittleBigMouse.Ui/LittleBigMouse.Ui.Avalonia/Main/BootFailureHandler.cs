#nullable enable
using System;
using System.Threading.Tasks;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// What happens when the bootstrapper faults.
/// <para>
/// Nothing observed a failed boot while the app was alive: the boot task was only awaited
/// after app.Run returned, so the process sat there with no window, no tray icon (the tray
/// is wired after the layout is built) and the single-instance lock held — every relaunch
/// signaled it and exited. That is "the GUI will no longer launch" next to "the process is
/// running in Task Manager" (#589, a registry key over 255 characters thrown by the first
/// layout load), and the only way out was Task Manager.
/// </para>
/// <para>
/// Three steps, each surviving the previous one's failure: log it (ui.log on Windows), show
/// it, shut the app down. The daemon is left alone on purpose, as on every other way the UI
/// goes away without the user choosing Exit: whatever it was doing before this failed start
/// is not this run's to undo, and the next successful start reconnects to it.
/// </para>
/// </summary>
internal sealed class BootFailureHandler(
    Action<string> log,
    Func<Exception, Task> showAsync,
    Action shutdown)
{
    public async Task HandleAsync(Exception error)
    {
        log($"Boot failed: {error}");

        try
        {
            await showAsync(error);
        }
        catch (Exception dialogError)
        {
            log($"Boot failure could not be shown: {dialogError}");
        }

        try
        {
            shutdown();
        }
        catch (Exception shutdownError)
        {
            log($"Shutdown after boot failure failed: {shutdownError}");
        }
    }
}
