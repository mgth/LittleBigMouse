#nullable enable
using System.Security.Authentication;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

static class VidaaCertificate
{
    public const string DefaultPassword = "186e990688070325a1c4b0ce275d2388";
    public static string ConfigurationDirectory => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "LittleBigMouse");
    public static string DefaultPath => Path.Combine(ConfigurationDirectory, "vidaa-client.p12");

    public static string Resolve(string configuredPath)
    {
        if (!string.IsNullOrWhiteSpace(configuredPath))
        {
            var expanded = configuredPath.StartsWith("~/", StringComparison.Ordinal)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), configuredPath[2..])
                : configuredPath;
            if (File.Exists(expanded)) return Path.GetFullPath(expanded);
        }
        foreach (var name in new[] { "vidaa-client.p12", "client_mobile_android.p12", "3R.p12" })
        {
            var candidate = Path.Combine(ConfigurationDirectory, name);
            if (File.Exists(candidate)) return candidate;
        }
        return "";
    }

    public static AuthenticationException MissingException(Exception? inner = null) => new(
        $"The C1 requires the VIDAA client certificate. Extract the .p12 from the official Android APK and copy it to {DefaultPath}.", inner);
}
