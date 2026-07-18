#nullable enable
using System;
using System.Linq;
using System.Text.RegularExpressions;
using LittleBigMouse.DisplayLayout.Monitors;
using Microsoft.Win32.TaskScheduler;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows autostart (Task Scheduler) for the layout. Lives in the Windows platform
/// project — the model is platform-neutral; a Linux head provides its own autostart.
/// Kept as extension methods on <see cref="IMonitorsLayout"/> so the existing call sites
/// (registry persistence, MainService) are unchanged apart from the namespace.
/// </summary>
public static class AutostartExtensions
{
    static string ServiceName =>
        "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent().Name.Replace('\\', '_');

    static string ApplicationExe =>
        AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName + ".exe";

    public static bool IsScheduled(this IMonitorsLayout layout)
    {
        // TEMP (Linux phase 1): Task Scheduler is Windows-only. Removed in phase 2 with the
        // persistence seam (Linux autostart = systemd user unit or .desktop file).
        if (!OperatingSystem.IsWindows()) return false;

        using var ts = new TaskService();
        return ts.RootFolder.GetTasks(new Regex(ServiceName)).Any();
    }

    /// <summary>
    /// Align the startup scheduled task on the current options.
    /// </summary>
    public static void UpdateSchedule(this IMonitorsLayout layout)
    {
        if (layout.Options.LoadAtStartup) layout.Schedule(layout.Options.StartElevated); else layout.Unschedule();
    }

    public static bool Schedule(this IMonitorsLayout layout, bool elevated)
    {
        // TEMP (Linux phase 1): see IsScheduled above.
        if (!OperatingSystem.IsWindows()) return false;

        layout.Unschedule();

        using var ts = new TaskService();

        var td = ts.NewTask();
        td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
        td.Triggers.Add(
            new LogonTrigger { UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name });

        td.Actions.Add(
            new ExecAction(ApplicationExe, "", AppDomain.CurrentDomain.BaseDirectory)
        );

        // Respect the requested level. The historical (2015) version tried Highest
        // FIRST regardless of the option — back then running as admin was the whole
        // point — so an elevated app kept re-registering an elevated task even with
        // StartElevated off, and the elevation self-perpetuated across restarts.
        td.Principal.RunLevel = elevated ? TaskRunLevel.Highest : TaskRunLevel.LUA;
        td.Settings.DisallowStartIfOnBatteries = false;
        td.Settings.DisallowStartOnRemoteAppSession = true;
        td.Settings.StopIfGoingOnBatteries = false;
        td.Settings.ExecutionTimeLimit = TimeSpan.Zero;
        try
        {
            ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
            return true;
        }
        catch (UnauthorizedAccessException)
        {
        }

        if (!elevated) return false;

        // Elevation requested but not allowed to register an elevated task:
        // degrade to a non-elevated autostart rather than none at all.
        td.Principal.RunLevel = TaskRunLevel.LUA;
        try
        {
            ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
            return true;
        }
        catch (UnauthorizedAccessException)
        {
            return false;
        }
    }

    public static void Unschedule(this IMonitorsLayout layout)
    {
        // TEMP (Linux phase 1): see IsScheduled above.
        if (!OperatingSystem.IsWindows()) return;

        using var ts = new TaskService();
        try
        {
            ts.RootFolder.DeleteTask(ServiceName, false);
        }
        catch (UnauthorizedAccessException)
        {
        }
    }
}
