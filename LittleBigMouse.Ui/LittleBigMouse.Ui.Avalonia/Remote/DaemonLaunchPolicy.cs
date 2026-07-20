#nullable enable
using System.Diagnostics;

namespace LittleBigMouse.Ui.Avalonia.Remote;

public static class DaemonLaunchPolicy
{
    public static ProcessStartInfo Create(string path, bool elevate)
        => new()
        {
            FileName = path,
            UseShellExecute = elevate,
            Verb = elevate ? "runas" : "",
            CreateNoWindow = !elevate,
        };
}
