#nullable enable
using System;
using System.Linq;
using System.Text.RegularExpressions;
using LittleBigMouse.DisplayLayout.Monitors;
using Microsoft.Win32.TaskScheduler;

namespace LittleBigMouse.Platform.Windows;

/// <summary>Per-user Windows Task Scheduler integration.</summary>
public static class AutostartExtensions
{
    static string ServiceName =>
        "LittleBigMouse_" + System.Security.Principal.WindowsIdentity.GetCurrent()
            .Name.Replace('\\', '_');

    static string ApplicationExe =>
        AppDomain.CurrentDomain.BaseDirectory + AppDomain.CurrentDomain.FriendlyName + ".exe";

    public static bool IsScheduled(this IMonitorsLayout layout)
    {
        if (!OperatingSystem.IsWindows()) return false;
        using var taskService = new TaskService();
        return taskService.RootFolder.GetTasks(new Regex(ServiceName)).Any();
    }

    public static void UpdateSchedule(this IMonitorsLayout layout)
    {
        if (layout.Options.LoadAtStartup) layout.Schedule(layout.Options.StartElevated);
        else layout.Unschedule();
    }

    public static bool Schedule(this IMonitorsLayout layout, bool elevateHook)
    {
        if (!OperatingSystem.IsWindows()) return false;
        layout.Unschedule();

        using var taskService = new TaskService();
        var definition = taskService.NewTask();
        definition.RegistrationInfo.Description = "Multi-DPI aware mouse transitions";
        definition.Triggers.Add(new LogonTrigger
        {
            UserId = System.Security.Principal.WindowsIdentity.GetCurrent().Name,
        });
        definition.Actions.Add(new ExecAction(ApplicationExe, "",
            AppDomain.CurrentDomain.BaseDirectory));

        // Autostart always launches the UI at normal integrity. StartElevated is
        // applied only to the hook daemon subsequently launched by that UI.
        definition.Principal.RunLevel = ScheduledUiRunLevel(elevateHook);
        definition.Settings.DisallowStartIfOnBatteries = false;
        definition.Settings.DisallowStartOnRemoteAppSession = true;
        definition.Settings.StopIfGoingOnBatteries = false;
        definition.Settings.ExecutionTimeLimit = TimeSpan.Zero;
        try
        {
            taskService.RootFolder.RegisterTaskDefinition(ServiceName, definition);
            return true;
        }
        catch (UnauthorizedAccessException)
        {
            return false;
        }
    }

    public static TaskRunLevel ScheduledUiRunLevel(bool elevateHook) => TaskRunLevel.LUA;

    public static void Unschedule(this IMonitorsLayout layout)
    {
        if (!OperatingSystem.IsWindows()) return;
        using var taskService = new TaskService();
        try { taskService.RootFolder.DeleteTask(ServiceName, exceptionOnNotExists: false); }
        catch (UnauthorizedAccessException) { }
    }
}
