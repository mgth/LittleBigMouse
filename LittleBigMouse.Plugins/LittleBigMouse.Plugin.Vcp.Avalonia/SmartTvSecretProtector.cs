#nullable enable
using System.Security.Cryptography;
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>Per-user at-rest protection for experimental smart-TV credentials.</summary>
public sealed class SmartTvSecretProtector
{
    const string DpapiPrefix = "dpapi:v1:";
    const string UnixPrefix = "aesgcm:v1:";
    static readonly byte[] Entropy = SHA256.HashData(
        Encoding.UTF8.GetBytes("LittleBigMouse/SmartTv/v1"));

    readonly string _keyPath;
    readonly object _keyLock = new();
    byte[]? _unixKey;

    public SmartTvSecretProtector(string directory)
        => _keyPath = Path.Combine(directory, ".smart-tv-secrets.key");

    public string Protect(string? plaintext)
    {
        if (string.IsNullOrEmpty(plaintext)) return string.Empty;
        var bytes = Encoding.UTF8.GetBytes(plaintext);
        if (OperatingSystem.IsWindows())
        {
            var protectedBytes = ProtectedData.Protect(
                bytes, Entropy, DataProtectionScope.CurrentUser);
            return DpapiPrefix + Convert.ToBase64String(protectedBytes);
        }

        var nonce = RandomNumberGenerator.GetBytes(12);
        var ciphertext = new byte[bytes.Length];
        var tag = new byte[16];
        using (var aes = new AesGcm(GetUnixKey(), tag.Length))
            aes.Encrypt(nonce, bytes, ciphertext, tag, Entropy);
        return UnixPrefix + Convert.ToBase64String([.. nonce, .. tag, .. ciphertext]);
    }

    public string Unprotect(string? protectedValue)
    {
        if (string.IsNullOrEmpty(protectedValue)) return string.Empty;
        if (protectedValue.StartsWith(DpapiPrefix, StringComparison.Ordinal))
        {
            if (!OperatingSystem.IsWindows())
                throw new CryptographicException("Windows-protected secret cannot be opened here.");
            var bytes = Convert.FromBase64String(protectedValue[DpapiPrefix.Length..]);
            return Encoding.UTF8.GetString(ProtectedData.Unprotect(
                bytes, Entropy, DataProtectionScope.CurrentUser));
        }
        if (protectedValue.StartsWith(UnixPrefix, StringComparison.Ordinal))
        {
            var payload = Convert.FromBase64String(protectedValue[UnixPrefix.Length..]);
            if (payload.Length < 28) throw new CryptographicException("Invalid secret envelope.");
            var plaintext = new byte[payload.Length - 28];
            using (var aes = new AesGcm(GetUnixKey(), 16))
                aes.Decrypt(payload.AsSpan(0, 12), payload.AsSpan(28),
                    payload.AsSpan(12, 16), plaintext, Entropy);
            return Encoding.UTF8.GetString(plaintext);
        }

        // One-time compatibility path: old settings were plaintext. The next save
        // rewrites them through Protect.
        return protectedValue;
    }

    byte[] GetUnixKey()
    {
        lock (_keyLock)
        {
            if (_unixKey is not null) return _unixKey;
            var directory = Path.GetDirectoryName(_keyPath)!;
            Directory.CreateDirectory(directory);
            if (!OperatingSystem.IsWindows())
                File.SetUnixFileMode(directory,
                    UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);

            if (File.Exists(_keyPath))
            {
                _unixKey = File.ReadAllBytes(_keyPath);
            }
            else
            {
                var key = RandomNumberGenerator.GetBytes(32);
                try
                {
                    using var stream = new FileStream(_keyPath, FileMode.CreateNew,
                        FileAccess.Write, FileShare.None);
                    stream.Write(key);
                    stream.Flush(flushToDisk: true);
                    if (!OperatingSystem.IsWindows())
                        File.SetUnixFileMode(_keyPath,
                            UnixFileMode.UserRead | UnixFileMode.UserWrite);
                    _unixKey = key;
                }
                catch (IOException) when (File.Exists(_keyPath))
                {
                    _unixKey = File.ReadAllBytes(_keyPath);
                }
            }

            if (_unixKey.Length != 32)
                throw new CryptographicException("Invalid smart-TV secret key.");
            return _unixKey;
        }
    }
}
