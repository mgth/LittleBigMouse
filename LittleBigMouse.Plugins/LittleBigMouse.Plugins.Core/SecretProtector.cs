#nullable enable
using System;
using System.IO;
using System.Security.Cryptography;
using System.Text;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Encrypts small secrets — smart-TV pairing tokens — before they reach the disk.
///
/// Windows uses DPAPI in <see cref="DataProtectionScope.CurrentUser"/> scope: the key is
/// derived from the logon credentials, so nothing extra has to be stored and another account
/// cannot read the file even with access to the profile directory. Unix has no equivalent
/// service, so a 32-byte random key lives in a <c>0600</c> file next to the data and the
/// payload is sealed with AES-GCM.
///
/// What this buys, on both systems: the token no longer sits in clear in a backup, a synced
/// profile, or a config directory someone widened the permissions on. What it does not buy:
/// protection from code running as you, which can read the key file — or ask DPAPI — exactly
/// as this application does. PRIVACY.md states the same limit.
///
/// Envelopes are self-describing (<c>LBM1.&lt;scheme&gt;.&lt;base64&gt;</c>), so a file
/// carried to a machine that cannot read it fails loudly instead of silently decoding to
/// rubbish, and files written in clear by 5.6.0 and earlier stay recognisable for migration.
/// </summary>
public sealed class SecretProtector
{
    const string Prefix = "LBM1.";
    const string DpapiScheme = "dpapi";
    const string AesGcmScheme = "aesgcm";

    const int KeyLength = 32;
    const int NonceLength = 12; // AesGcm.NonceByteSizes.MaxSize
    const int TagLength = 16;   // AesGcm.TagByteSizes.MaxSize

    readonly string _keyFilePath;
    readonly object _keyLock = new();
    byte[]? _key;

    public SecretProtector(string keyFilePath) => _keyFilePath = keyFilePath;

    /// <summary>
    /// The protector guarding <paramref name="dataFilePath"/>: on Unix its key sits beside the
    /// data, so every store in the configuration directory shares one key file — and a store
    /// pointed at a temporary directory (tests) gets its own, never touching the real profile.
    /// </summary>
    public static SecretProtector ForFile(string dataFilePath)
        => new(Path.Combine(
            Path.GetDirectoryName(Path.GetFullPath(dataFilePath))!, "secrets.key"));

    /// <summary>Whether <paramref name="content"/> is one of our envelopes rather than clear text.</summary>
    public static bool IsProtected(string content) => content.StartsWith(Prefix, StringComparison.Ordinal);

    public string Protect(string plainText)
    {
        var bytes = Encoding.UTF8.GetBytes(plainText);

        // The OS check has to be on this line: it is what lets the platform analyser see that
        // the Windows-only call below is guarded.
        if (OperatingSystem.IsWindows())
            return Prefix + DpapiScheme + "."
                 + Convert.ToBase64String(ProtectedData.Protect(bytes, null, DataProtectionScope.CurrentUser));

        // nonce ‖ tag ‖ ciphertext, so the whole envelope is one base64 blob.
        var payload = new byte[NonceLength + TagLength + bytes.Length];
        var nonce = payload.AsSpan(0, NonceLength);
        RandomNumberGenerator.Fill(nonce);

        using var aes = new AesGcm(GetOrCreateKey(), TagLength);
        aes.Encrypt(
            nonce,
            bytes,
            payload.AsSpan(NonceLength + TagLength),
            payload.AsSpan(NonceLength, TagLength));

        return Prefix + AesGcmScheme + "." + Convert.ToBase64String(payload);
    }

    /// <summary>
    /// Reverses <see cref="Protect"/>. Throws — on a foreign scheme, a wrong key, a truncated
    /// or tampered payload — rather than returning something plausible; callers treat that the
    /// same way they already treat unreadable settings, by starting fresh.
    /// </summary>
    public string Unprotect(string envelope)
    {
        if (!IsProtected(envelope))
            throw new CryptographicException("Not a protected payload.");

        var body = envelope.AsSpan(Prefix.Length);
        var separator = body.IndexOf('.');
        if (separator < 0) throw new CryptographicException("Protected payload has no scheme.");

        var scheme = body[..separator].ToString();
        var payload = Convert.FromBase64String(body[(separator + 1)..].ToString());

        switch (scheme)
        {
            case DpapiScheme when OperatingSystem.IsWindows():
                return Encoding.UTF8.GetString(
                    ProtectedData.Unprotect(payload, null, DataProtectionScope.CurrentUser));

            case AesGcmScheme:
                if (payload.Length < NonceLength + TagLength)
                    throw new CryptographicException("Protected payload is truncated.");

                var plain = new byte[payload.Length - NonceLength - TagLength];
                using (var aes = new AesGcm(ReadKey() ?? throw new CryptographicException(
                           $"No key at {_keyFilePath} to read this payload with."), TagLength))
                {
                    aes.Decrypt(
                        payload.AsSpan(0, NonceLength),
                        payload.AsSpan(NonceLength + TagLength),
                        payload.AsSpan(NonceLength, TagLength),
                        plain);
                }
                return Encoding.UTF8.GetString(plain);

            default:
                throw new CryptographicException(
                    $"Payload was protected with '{scheme}', unreadable on this system.");
        }
    }

    byte[] GetOrCreateKey()
    {
        lock (_keyLock)
        {
            return _key ??= ReadKeyLocked() ?? CreateKeyLocked();
        }
    }

    byte[]? ReadKey()
    {
        lock (_keyLock)
        {
            return _key ??= ReadKeyLocked();
        }
    }

    byte[]? ReadKeyLocked()
    {
        if (!File.Exists(_keyFilePath)) return null;

        byte[] key;
        try
        {
            key = Convert.FromBase64String(File.ReadAllText(_keyFilePath).Trim());
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Secret key {_keyFilePath} is unreadable ({e.Message}); replacing it. Paired televisions must be paired again.");
            return null;
        }

        if (key.Length != KeyLength)
        {
            Console.Error.WriteLine($"Secret key {_keyFilePath} has the wrong length; replacing it. Paired televisions must be paired again.");
            return null;
        }

        // A key restored from a backup, or written before this ran, may be group- or
        // world-readable. Narrow it back down.
        if (!OperatingSystem.IsWindows())
        {
            try { File.SetUnixFileMode(_keyFilePath, UnixFileMode.UserRead | UnixFileMode.UserWrite); }
            catch (Exception e) { Console.Error.WriteLine($"Could not restrict {_keyFilePath}: {e.Message}"); }
        }

        return key;
    }

    byte[] CreateKeyLocked()
    {
        var key = RandomNumberGenerator.GetBytes(KeyLength);

        var directory = Path.GetDirectoryName(_keyFilePath);
        if (!string.IsNullOrEmpty(directory)) Directory.CreateDirectory(directory);

        // CreateNew + UnixCreateMode: the key is never briefly world-readable, and a second
        // instance racing us here loses the create and reuses the key that won.
        var options = new FileStreamOptions { Mode = FileMode.CreateNew, Access = FileAccess.Write };
        if (!OperatingSystem.IsWindows())
            options.UnixCreateMode = UnixFileMode.UserRead | UnixFileMode.UserWrite;

        try
        {
            using var stream = new FileStream(_keyFilePath, options);
            using var writer = new StreamWriter(stream);
            writer.Write(Convert.ToBase64String(key));
        }
        catch (IOException) when (File.Exists(_keyFilePath))
        {
            var winner = ReadKeyLocked();
            if (winner is null) throw;
            return winner;
        }

        return key;
    }
}
