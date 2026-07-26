using System.Diagnostics;

namespace LittleBigMouse.Ui.Avalonia;

public static class SupportLinks
{
    public const string KofiUrl = "https://ko-fi.com/mgth";

    public static void OpenKofi()
    {
        // UseShellExecute routes to the OS opener (ShellExecute / xdg-open).
        Process.Start(new ProcessStartInfo(KofiUrl) { UseShellExecute = true });
    }
}
