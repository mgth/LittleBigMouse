#nullable enable
using System.Net.Security;
using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>Certificate SHA-256 pin validator with explicit pairing/TOFU mode.</summary>
public sealed class DeviceCertificatePin(string? expectedFingerprint, bool allowNewFingerprint)
{
    readonly string _expected = Normalize(expectedFingerprint);
    public string ObservedFingerprint { get; private set; } = "";

    public bool Validate(
        object sender,
        X509Certificate? certificate,
        X509Chain? chain,
        SslPolicyErrors errors)
    {
        if (certificate is null) return false;
        var raw = certificate.GetRawCertData();
        ObservedFingerprint = Convert.ToHexString(SHA256.HashData(raw));
        if (allowNewFingerprint) return true;
        if (_expected.Length != 64) return false;

        return CryptographicOperations.FixedTimeEquals(
            Convert.FromHexString(_expected),
            Convert.FromHexString(ObservedFingerprint));
    }

    public static string Display(string? fingerprint)
    {
        var normalized = Normalize(fingerprint);
        return normalized.Length == 64
            ? string.Join(':', Enumerable.Range(0, 32)
                .Select(index => normalized.Substring(index * 2, 2)))
            : "unavailable";
    }

    static string Normalize(string? value)
        => new((value ?? "").Where(char.IsAsciiHexDigit)
            .Select(char.ToUpperInvariant).ToArray());
}
