namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// A test that only runs on Windows and is reported as skipped elsewhere.
/// <para>
/// For the registry store there is no alternative: <c>Microsoft.Win32.Registry</c> throws
/// <c>PlatformNotSupportedException</c> off Windows, and no in-memory stand-in would be
/// testing the thing that breaks — the key names and the value encoding are the format.
/// The CI job runs the whole suite on <c>windows-latest</c>, so these do run before a
/// merge; they are the tests a Linux workstation cannot give you.
/// </para>
/// </summary>
public sealed class WindowsFactAttribute : FactAttribute
{
    public WindowsFactAttribute()
    {
        if (!OperatingSystem.IsWindows())
            Skip = "Windows only: the registry store talks to HKCU (runs in CI on windows-latest).";
    }
}
