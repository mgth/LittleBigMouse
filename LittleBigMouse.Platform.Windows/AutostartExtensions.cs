#nullable enable
using System;
using System.IO;
using System.Linq;
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
        // By name, not by a pattern: an account name is not a regex.
        using var task = ts.GetTask(ServiceName);
        if (task == null) return false;
        if (LaunchesExistingProgram(task.Definition)) return true;

        // The task is the user's, not an install's: one registered by a copy since moved
        // or uninstalled is still there, so the option reads on, but every logon fails
        // with ERROR_FILE_NOT_FOUND (0x80070002) and nothing ever rewrites it — the
        // toggle shows nothing to fix. Point it back at this executable, registered
        // afresh rather than edited: a definition read back carries the account as a
        // SID, which RegisterTaskDefinition rejects ("UserId").
        var elevated = task.Definition.Principal.RunLevel == TaskRunLevel.Highest;
        try
        {
            // False when it could not be rewritten (an elevated task, from a non-elevated
            // process): the option then reads off, which is what logons get.
            return layout.Schedule(elevated);
        }
        catch (Exception error)
        {
            // Never keep the layout from loading.
            Console.Error.WriteLine($"[LittleBigMouse] Autostart task retarget failed: {error.Message}");
            return false;
        }
    }

    static bool LaunchesExistingProgram(TaskDefinition td)
    {
        var actions = td.Actions.OfType<ExecAction>().ToList();
        return actions.Count > 0 && actions.All(a => File.Exists(a.Path?.Trim('"')));
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
