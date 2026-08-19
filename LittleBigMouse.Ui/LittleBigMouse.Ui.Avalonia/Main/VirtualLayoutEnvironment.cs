using System;
using System.IO;
using System.IO.Compression;
using System.Text;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Debug hook: <c>LBM_VIRTUAL_LAYOUT=path-to-export(.json|.gz)</c> replaces the system layout
/// with a virtual one, so a user's export can be inspected straight from the command line —
/// same display-only semantics as the "open virtual layout" debug button.
/// </summary>
public static class VirtualLayoutEnvironment
{
    public const string Variable = "LBM_VIRTUAL_LAYOUT";

    /// <summary>
    /// The layout named by the environment variable, or null when it is unset or unreadable.
    /// A bad path is reported and ignored: this is a diagnostic aid, and one that refused to
    /// start the app would be a poor one.
    /// </summary>
    public static IMonitorsLayout? Load()
    {
        var path = Environment.GetEnvironmentVariable(Variable);
        if (string.IsNullOrWhiteSpace(path)) return null;

        try
        {
            return VirtualLayoutFactory.FromJson(ReadJson(path), LayoutSource.VirtualFile, path);
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"{Variable}: failed to load '{path}': {ex}");
            return null;
        }
    }

    /// <summary>Reads the export, transparently un-gzipping one that was saved compressed.</summary>
    static string ReadJson(string path)
    {
        var bytes = File.ReadAllBytes(path);

        if (bytes.Length <= 2 || bytes[0] != 0x1f || bytes[1] != 0x8b)
            return Encoding.UTF8.GetString(bytes);

        using var gzip = new GZipStream(new MemoryStream(bytes), CompressionMode.Decompress);
        using var reader = new StreamReader(gzip);
        return reader.ReadToEnd();
    }
}
