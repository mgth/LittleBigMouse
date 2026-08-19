using System;
using System.IO;
using LittleBigMouse.Plugins;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Rolling trace of daemon events.
/// <para>
/// Display-event storms — a wake from sleep, an HDR TV flapping its DPMS state — rebuild the
/// layout every few seconds for hours, and identifying which event was looping is otherwise
/// impossible after the fact: by the time anyone looks, all that is left is the symptom.
/// </para>
/// </summary>
public static class DaemonEventTrace
{
    const long MaxBytes = 1_000_000;

    public static void Write(LittleBigMouseEvent daemonEvent)
    {
        try
        {
            var dir = LbmPaths.DataDir;
            Directory.CreateDirectory(dir);
            var file = Path.Combine(dir, "daemon-events.log");

            if (File.Exists(file) && new FileInfo(file).Length > MaxBytes)
                File.WriteAllText(file, "");

            File.AppendAllText(file, $"{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff} {daemonEvent}\r\n");
        }
        catch
        {
            // tracing must never take the app down
        }
    }
}
