using System.Diagnostics;
// ReSharper disable once InconsistentNaming
const string APP_GUID = "51B5711E-1A7F-436E-B3DD-B598901B3FD2";

var verb = args.Any(a => a.ToLower().Contains("-elevated")) ? "runas" : "";

var startInfo = new ProcessStartInfo
{
    FileName = "LittleBigMouse.Ui.Avalonia.exe", 
    Verb = verb, // "runas" to elevate
    UseShellExecute = true
};

try
{
    // wait for gui to stop
    using (var mutex = new Mutex(true, APP_GUID))
    {
        while (!mutex.WaitOne(TimeSpan.FromSeconds(1), false))
        {
        }
    }

    Process.Start(startInfo);
}
catch (System.ComponentModel.Win32Exception)
{
    Console.WriteLine("L'élévation des privilèges a été annulée par l'utilisateur.");
}
