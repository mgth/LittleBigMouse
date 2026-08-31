using System;
using System.Collections.Generic;
using System.Diagnostics;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Production <see cref="IProcessHost"/>: a thin wrapper over <see cref="Process"/>. All the real
/// OS calls live here so <see cref="DaemonProcessManager"/> stays testable.
/// </summary>
public sealed class SystemProcessHost : IProcessHost
{
    public int CurrentSessionId
    {
        get
        {
            using var self = Process.GetCurrentProcess();
            return self.SessionId;
        }
    }

    public IEnumerable<IDaemonProcess> Enumerate(IEnumerable<string> processNames)
    {
        foreach (var name in processNames)
        foreach (var process in Process.GetProcessesByName(name))
            yield return new SystemDaemonProcess(process);
    }

    public IDaemonProcess Launch(string path)
    {
        // Elevation model (#512): the UI itself runs elevated when the user opted in, so the daemon
        // inherits that level — no runas here.
        var startInfo = new ProcessStartInfo
        {
            FileName = path,
#if DEBUG
            UseShellExecute = true,
#else
            UseShellExecute = false,
            CreateNoWindow = true,
#endif
        };

        var process = new Process { StartInfo = startInfo };
        process.Start();
        return new SystemDaemonProcess(process);
    }
}

/// <summary>Owns one real <see cref="Process"/> handle.</summary>
sealed class SystemDaemonProcess(Process process) : IDaemonProcess
{
    public bool HasExited => process.HasExited;
    public int SessionId => process.SessionId;
    public int Id => process.Id;
    public string ProcessName => process.ProcessName;

    public bool TryStop()
    {
        try
        {
            if (process.HasExited) return true;
            process.Kill(entireProcessTree: true);
            return process.WaitForExit(2000);
        }
        catch (Exception error) when (error is InvalidOperationException
                                      or System.ComponentModel.Win32Exception
                                      or NotSupportedException)
        {
            Debug.WriteLine($"Could not force-stop {process.ProcessName}: {error.Message}");
            return false;
        }
    }

    public void Dispose() => process.Dispose();
}
