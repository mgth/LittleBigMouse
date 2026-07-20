#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json.Nodes;

namespace LittleBigMouse.Ui.Avalonia.Updater;

public sealed record ReleaseUpdateInfo(
    Version Version,
    string Message,
    Uri DownloadUri,
    string FileName,
    long Size,
    string Sha256);

public static class ReleaseUpdateSecurity
{
    public const string ExpectedPublisher = "SignPath Foundation";
    const long MaximumInstallerSize = 512L * 1024 * 1024;

    public static ReleaseUpdateInfo? SelectNewest(JsonNode? document)
    {
        if (document is not JsonArray releases) return null;
        var candidates = new List<ReleaseUpdateInfo>();
        foreach (var node in releases)
        {
            if (node is not JsonObject release
                || !TryReadOptionalBoolean(release, "draft", out var draft)
                || !TryReadOptionalBoolean(release, "prerelease", out var prerelease)
                || draft
                || prerelease)
                continue;

            if (!TryParseVersion(ReadString(release, "tag_name"), out var version)
                && !TryParseVersion(ReadString(release, "name"), out version))
                continue;
            if (release["assets"] is not JsonArray assets) continue;

            var expectedName = $"LittleBigMouse_{version}.exe";
            var matching = assets.OfType<JsonObject>()
                .Where(asset => ReadString(asset, "name")
                    ?.Equals(expectedName, StringComparison.OrdinalIgnoreCase) == true)
                .ToArray();
            if (matching.Length != 1) continue;
            var asset = matching[0];
            if (ReadString(asset, "state") is { } state
                && !state.Equals("uploaded", StringComparison.OrdinalIgnoreCase))
                continue;
            if (!TryReadDigest(ReadString(asset, "digest"), out var digest)) continue;
            if (!TryReadInt64(asset, "size", out var size)
                || size is <= 0 or > MaximumInstallerSize)
                continue;
            if (!Uri.TryCreate(ReadString(asset, "browser_download_url"),
                    UriKind.Absolute, out var uri)
                || uri.Scheme != Uri.UriSchemeHttps
                || !uri.Host.Equals("github.com", StringComparison.OrdinalIgnoreCase)
                || !uri.AbsolutePath.StartsWith(
                    "/mgth/LittleBigMouse/releases/download/",
                    StringComparison.OrdinalIgnoreCase))
                continue;

            candidates.Add(new ReleaseUpdateInfo(version,
                ReadString(release, "body") ?? "", uri, expectedName, size, digest));
        }
        return candidates.OrderByDescending(candidate => candidate.Version).FirstOrDefault();
    }

    public static bool VerifySha256(string path, string expectedHex)
    {
        if (!TryReadDigest("sha256:" + expectedHex, out var normalized)) return false;
        using var stream = File.OpenRead(path);
        var actual = SHA256.HashData(stream);
        return CryptographicOperations.FixedTimeEquals(
            actual, Convert.FromHexString(normalized));
    }

    public static bool TryParseVersion(string? text, out Version version)
    {
        version = new Version();
        if (string.IsNullOrWhiteSpace(text)) return false;
        text = text.Trim();
        if (text[0] is 'v' or 'V') text = text[1..];
        return Version.TryParse(text, out version!);
    }

    static bool TryReadDigest(string? value, out string digest)
    {
        digest = "";
        const string prefix = "sha256:";
        if (value is null || !value.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
            return false;
        var candidate = value[prefix.Length..];
        if (candidate.Length != 64 || candidate.Any(c => !char.IsAsciiHexDigit(c))) return false;
        digest = candidate.ToUpperInvariant();
        return true;
    }

    static string? ReadString(JsonObject node, string name)
        => node[name] is JsonValue value && value.TryGetValue<string>(out var result)
            ? result
            : null;

    static bool TryReadInt64(JsonObject node, string name, out long result)
    {
        result = 0;
        return node[name] is JsonValue value && value.TryGetValue(out result);
    }

    static bool TryReadOptionalBoolean(JsonObject node, string name, out bool result)
    {
        result = false;
        return node[name] is null
               || node[name] is JsonValue value && value.TryGetValue(out result);
    }
}

public static class AuthenticodeVerifier
{
    static readonly Guid GenericVerifyV2 = new("00AAC56B-CD44-11d0-8CC2-00C04FC295EE");

    public static bool IsTrustedPublisher(string path, string expectedPublisher)
    {
        if (!OperatingSystem.IsWindows() || !File.Exists(path)) return false;
        if (WinVerifyTrust(path) != 0) return false;
        try
        {
#pragma warning disable SYSLIB0057 // Authenticode has no X509CertificateLoader signed-file equivalent.
            using var certificate = new X509Certificate2(
                X509Certificate.CreateFromSignedFile(path));
#pragma warning restore SYSLIB0057
            return certificate.GetNameInfo(X509NameType.SimpleName, forIssuer: false)
                .Equals(expectedPublisher, StringComparison.Ordinal);
        }
        catch (CryptographicException)
        {
            return false;
        }
    }

    static int WinVerifyTrust(string path)
    {
        var fileInfo = new WinTrustFileInfo(path);
        var fileInfoPointer = Marshal.AllocHGlobal(Marshal.SizeOf<WinTrustFileInfo>());
        var dataPointer = Marshal.AllocHGlobal(Marshal.SizeOf<WinTrustData>());
        try
        {
            Marshal.StructureToPtr(fileInfo, fileInfoPointer, fDeleteOld: false);
            var data = new WinTrustData(fileInfoPointer);
            Marshal.StructureToPtr(data, dataPointer, fDeleteOld: false);
            return NativeWinVerifyTrust(IntPtr.Zero, GenericVerifyV2, dataPointer);
        }
        finally
        {
            Marshal.FreeHGlobal(dataPointer);
            Marshal.FreeHGlobal(fileInfoPointer);
        }
    }

    [DllImport("wintrust.dll", EntryPoint = "WinVerifyTrust",
        ExactSpelling = true, CharSet = CharSet.Unicode)]
    static extern int NativeWinVerifyTrust(
        IntPtr window,
        [MarshalAs(UnmanagedType.LPStruct)] Guid action,
        IntPtr data);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    readonly struct WinTrustFileInfo
    {
        readonly uint _size;
        [MarshalAs(UnmanagedType.LPWStr)] readonly string _path;
        readonly IntPtr _file;
        readonly IntPtr _knownSubject;

        public WinTrustFileInfo(string path)
        {
            _size = (uint)Marshal.SizeOf<WinTrustFileInfo>();
            _path = path;
            _file = IntPtr.Zero;
            _knownSubject = IntPtr.Zero;
        }
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    readonly struct WinTrustData
    {
        readonly uint _size;
        readonly IntPtr _policyCallbackData;
        readonly IntPtr _sipClientData;
        readonly uint _uiChoice;
        readonly uint _revocationChecks;
        readonly uint _unionChoice;
        readonly IntPtr _fileInfo;
        readonly uint _stateAction;
        readonly IntPtr _stateData;
        readonly IntPtr _urlReference;
        readonly uint _providerFlags;
        readonly uint _uiContext;
        readonly IntPtr _signatureSettings;

        public WinTrustData(IntPtr fileInfo)
        {
            _size = (uint)Marshal.SizeOf<WinTrustData>();
            _policyCallbackData = IntPtr.Zero;
            _sipClientData = IntPtr.Zero;
            _uiChoice = 2; // WTD_UI_NONE
            _revocationChecks = 1; // WTD_REVOKE_WHOLECHAIN
            _unionChoice = 1; // WTD_CHOICE_FILE
            _fileInfo = fileInfo;
            _stateAction = 0; // WTD_STATEACTION_IGNORE
            _stateData = IntPtr.Zero;
            _urlReference = IntPtr.Zero;
            _providerFlags = 0x80; // WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT
            _uiContext = 0;
            _signatureSettings = IntPtr.Zero;
        }
    }
}
