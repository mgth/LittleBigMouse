#nullable enable
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Runtime.Versioning;
using Microsoft.Win32;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Opportunistic startup elevation (#512, #400). The app manifest is asInvoker so the
/// app STARTS for everyone — including standard users, for whom the former
/// requireAdministrator manifest meant an unanswerable UAC credential prompt and the
/// app never launching at all. Elevation is now taken only when it is both wanted
/// (the "Start elevated" option) and possible (a split-token administrator): one UAC
/// consent prompt via a self-relaunch. Standard users just run non-elevated — their
/// only limitation is UIPI: the router cannot inject input while an ELEVATED window
/// holds the focus, which a standard user rarely has in his session anyway.
/// Scheduled-task logons don't go through this: the task itself runs elevated
/// (TaskRunLevel.Highest) without any prompt for admins.
/// </summary>
[SupportedOSPlatform("windows")]
public static class WindowsElevation
{
    const string RootKey = @"SOFTWARE\Mgth\LittleBigMouse";

    /// <summary>
    /// True when the persisted "Start elevated" option is set, the current process is
    /// not elevated, and the user actually CAN elevate (split-token administrator).
    /// Must be evaluated before the single-instance guard: the relaunched process
    /// takes the lock the moment the current one returns from Main.
    /// </summary>
    public static bool ShouldRelaunchElevated()
    {
        if (Environment.IsPrivilegedProcess) return false;
        if (!StartElevatedRequested()) return false;
        // A standard user gets a credential prompt he cannot answer: never auto-prompt
        // him, run non-elevated instead (the app works, see UIPI note above).
        return TokenElevationType() == TOKEN_ELEVATION_TYPE_LIMITED;
    }

    /// <summary>
    /// Relaunch the current executable elevated (one UAC consent). Returns true when
    /// the elevated instance is on its way — the caller must exit WITHOUT taking the
    /// single-instance lock. A declined UAC returns false: keep running non-elevated.
    /// </summary>
    public static bool RelaunchElevated(string[] args)
    {
        var exe = Environment.ProcessPath;
        if (exe is null) return false;
        try
        {
            var psi = new ProcessStartInfo(exe)
            {
                UseShellExecute = true,
                Verb = "runas",
                WorkingDirectory = AppDomain.CurrentDomain.BaseDirectory,
            };
            foreach (var a in args) psi.ArgumentList.Add(a);
            return Process.Start(psi) != null;
        }
        catch (Win32Exception)
        {
            // ERROR_CANCELLED: the user said no. Non-elevated it is.
            return false;
        }
    }

    static bool StartElevatedRequested()
    {
        try
        {
            using var root = Registry.CurrentUser.OpenSubKey(RootKey);
            // Same encoding as RegistryExt.TryGetBool ("1"/"0" strings); the legacy
            // per-layout location is not worth chasing this early — it migrates to
            // the root key at the first save anyway.
            return root?.GetValue("StartElevated") as string == "1";
        }
        catch
        {
            return false;
        }
    }

    // GetTokenInformation(TokenElevationType): Default = no split token (standard user,
    // or UAC off), Full = elevated, Limited = filtered admin token — UAC can elevate.
    const int TOKEN_ELEVATION_TYPE_LIMITED = 3;

    static int TokenElevationType()
    {
        const uint TOKEN_QUERY = 0x0008;
        const int TokenElevationTypeClass = 18;

        if (!OpenProcessToken(Process.GetCurrentProcess().Handle, TOKEN_QUERY, out var token))
            return 0;
        try
        {
            return GetTokenInformation(token, TokenElevationTypeClass, out var value, sizeof(int), out _)
                ? value
                : 0;
        }
        finally
        {
            CloseHandle(token);
        }
    }

    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool OpenProcessToken(nint processHandle, uint desiredAccess, out nint tokenHandle);

    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool GetTokenInformation(nint tokenHandle, int informationClass, out int information, int informationLength, out int returnLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(nint handle);
}
