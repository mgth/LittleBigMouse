using System.Security.Cryptography;
using System.Text.Json;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using LittleBigMouse.Plugins;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// Pairing tokens are credentials: they must not be legible on disk, and the files written in
/// clear by 5.6.0 and earlier must survive the upgrade rather than silently losing pairings.
/// </summary>
public class SecretStorageTests : IDisposable
{
    readonly string _directory = Path.Combine(
        Path.GetTempPath(), "lbm-secret-tests", Guid.NewGuid().ToString("N"));

    public SecretStorageTests() => Directory.CreateDirectory(_directory);

    public void Dispose()
    {
        if (Directory.Exists(_directory)) Directory.Delete(_directory, recursive: true);
    }

    string At(string name) => Path.Combine(_directory, name);

    // -- SecretProtector ---------------------------------------------------------------

    [Fact]
    public void ProtectedPayloadRoundTripsAndHidesThePlainText()
    {
        var protector = SecretProtector.ForFile(At("data.json"));
        var envelope = protector.Protect("""{"Token":"pairing-token-1234"}""");

        Assert.True(SecretProtector.IsProtected(envelope));
        Assert.DoesNotContain("pairing-token-1234", envelope);
        Assert.Equal("""{"Token":"pairing-token-1234"}""", protector.Unprotect(envelope));
    }

    [Fact]
    public void ProtectIsNotDeterministic()
    {
        var protector = SecretProtector.ForFile(At("data.json"));

        // Same token twice must not produce the same bytes, or the file would leak that a
        // pairing was re-saved unchanged.
        Assert.NotEqual(protector.Protect("token"), protector.Protect("token"));
    }

    [Fact]
    public void ClearTextIsNotMistakenForAnEnvelope()
    {
        Assert.False(SecretProtector.IsProtected("""{"SAM123":{"Token":"secret"}}"""));
        Assert.False(SecretProtector.IsProtected(""));
    }

    [Fact]
    public void TamperedPayloadIsRejectedRatherThanDecoded()
    {
        var protector = SecretProtector.ForFile(At("data.json"));
        var envelope = protector.Protect("token");

        // Flip a bit in the base64 body; AES-GCM's tag has to notice.
        var body = envelope[(envelope.LastIndexOf('.') + 1)..];
        var bytes = Convert.FromBase64String(body);
        bytes[^1] ^= 0x01;
        var tampered = envelope[..(envelope.LastIndexOf('.') + 1)] + Convert.ToBase64String(bytes);

        Assert.ThrowsAny<CryptographicException>(() => protector.Unprotect(tampered));
    }

    [Fact]
    public void PayloadFromAnotherKeyIsRejected()
    {
        var mine = SecretProtector.ForFile(At("data.json"));
        var envelope = mine.Protect("token");

        var elsewhere = Directory.CreateDirectory(At("other")).FullName;
        var theirs = SecretProtector.ForFile(Path.Combine(elsewhere, "data.json"));

        Assert.ThrowsAny<CryptographicException>(() => theirs.Unprotect(envelope));
    }

    [Fact]
    public void PayloadFromAnUnreadableSchemeIsRejected()
    {
        var protector = SecretProtector.ForFile(At("data.json"));

        // A DPAPI envelope read on Unix, or anything from a future version: refuse it, never
        // hand back a plausible-looking string.
        Assert.ThrowsAny<CryptographicException>(
            () => protector.Unprotect("LBM1.rot13." + Convert.ToBase64String("token"u8.ToArray())));
        Assert.ThrowsAny<CryptographicException>(() => protector.Unprotect("LBM1.no-separator-body"));
        Assert.ThrowsAny<CryptographicException>(() => protector.Unprotect("plain text"));
    }

    [Fact]
    public void KeyFileIsOwnerOnlyAndReused()
    {
        if (OperatingSystem.IsWindows()) return; // DPAPI; no key file exists to check.

        var protector = SecretProtector.ForFile(At("data.json"));
        var envelope = protector.Protect("token");

        var keyFile = At("secrets.key");
        Assert.True(File.Exists(keyFile));
        Assert.Equal(UnixFileMode.UserRead | UnixFileMode.UserWrite, File.GetUnixFileMode(keyFile));

        // A second protector over the same directory reuses the key rather than replacing it.
        Assert.Equal("token", SecretProtector.ForFile(At("data.json")).Unprotect(envelope));
    }

    [Fact]
    public void LooseKeyFilePermissionsAreNarrowedOnRead()
    {
        if (OperatingSystem.IsWindows()) return;

        var keyFile = At("secrets.key");
        SecretProtector.ForFile(At("data.json")).Protect("token");
        File.SetUnixFileMode(keyFile, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.GroupRead | UnixFileMode.OtherRead);

        SecretProtector.ForFile(At("data.json")).Protect("token");

        Assert.Equal(UnixFileMode.UserRead | UnixFileMode.UserWrite, File.GetUnixFileMode(keyFile));
    }

    // -- Samsung Tizen -----------------------------------------------------------------

    [Fact]
    public void SamsungTokenIsNotLegibleOnDisk()
    {
        var path = At("samsung-tizen.json");
        new SamsungTizenSettingsStore(path).Save(new SamsungTizenConfiguration
        {
            MonitorId = "SAM123",
            IpAddress = "192.168.1.42",
            Token = "pairing-token-1234",
        });

        var onDisk = File.ReadAllText(path);
        Assert.True(SecretProtector.IsProtected(onDisk));
        Assert.DoesNotContain("pairing-token-1234", onDisk);
        Assert.DoesNotContain("192.168.1.42", onDisk);

        Assert.Equal("pairing-token-1234", new SamsungTizenSettingsStore(path).Get("SAM123")!.Token);
    }

    [Fact]
    public void SamsungClearTextFileIsReadOnceThenRewrittenEncrypted()
    {
        var path = At("samsung-tizen.json");
        File.WriteAllText(path, JsonSerializer.Serialize(new Dictionary<string, SamsungTizenConfiguration>
        {
            ["SAM123"] = new() { MonitorId = "SAM123", IpAddress = "192.168.1.42", Token = "legacy-token" },
        }));

        // Reading is enough to trigger the migration: the pairing survives...
        Assert.Equal("legacy-token", new SamsungTizenSettingsStore(path).Get("SAM123")!.Token);

        // ...and the clear-text copy is gone from the file.
        var onDisk = File.ReadAllText(path);
        Assert.True(SecretProtector.IsProtected(onDisk));
        Assert.DoesNotContain("legacy-token", onDisk);

        Assert.Equal("legacy-token", new SamsungTizenSettingsStore(path).Get("SAM123")!.Token);
    }

    [Fact]
    public void SamsungUndecryptableFileStartsFreshInsteadOfThrowing()
    {
        var path = At("samsung-tizen.json");
        File.WriteAllText(path, "LBM1.aesgcm." + Convert.ToBase64String(new byte[64]));

        var store = new SamsungTizenSettingsStore(path);
        Assert.Null(store.Get("SAM123"));

        // Still usable afterwards — the user re-pairs and it saves.
        store.Save(new SamsungTizenConfiguration { MonitorId = "SAM123", Token = "fresh" });
        Assert.Equal("fresh", new SamsungTizenSettingsStore(path).Get("SAM123")!.Token);
    }

    // -- Hisense VIDAA -----------------------------------------------------------------

    [Fact]
    public void HisenseTokensAreNotLegibleOnDisk()
    {
        var path = At("hisense-vidaa.json");
        new HisenseVidaaSettingsStore(path).Save(new HisenseVidaaConfiguration
        {
            MonitorId = "HIS123",
            IpAddress = "192.168.1.43",
            AccessToken = "access-token-1234",
            RefreshToken = "refresh-token-5678",
        });

        var onDisk = File.ReadAllText(path);
        Assert.True(SecretProtector.IsProtected(onDisk));
        Assert.DoesNotContain("access-token-1234", onDisk);
        Assert.DoesNotContain("refresh-token-5678", onDisk);

        var loaded = new HisenseVidaaSettingsStore(path).Get("HIS123")!;
        Assert.Equal("access-token-1234", loaded.AccessToken);
        Assert.Equal("refresh-token-5678", loaded.RefreshToken);
    }

    [Fact]
    public void HisenseRoundTripDoesNotShareMutableInstances()
    {
        var path = At("hisense-vidaa.json");
        var store = new HisenseVidaaSettingsStore(path);
        var configuration = new HisenseVidaaConfiguration { MonitorId = "HIS123", AccessToken = "secret" };

        store.Save(configuration);
        configuration.AccessToken = "changed";

        Assert.Equal("secret", store.Get("HIS123")!.AccessToken);
        Assert.Equal("secret", new HisenseVidaaSettingsStore(path).Get("HIS123")!.AccessToken);
    }

    [Fact]
    public void HisenseClearTextFileIsReadOnceThenRewrittenEncrypted()
    {
        var path = At("hisense-vidaa.json");
        File.WriteAllText(path, JsonSerializer.Serialize(new Dictionary<string, HisenseVidaaConfiguration>
        {
            ["HIS123"] = new() { MonitorId = "HIS123", AccessToken = "legacy-token" },
        }));

        Assert.Equal("legacy-token", new HisenseVidaaSettingsStore(path).Get("HIS123")!.AccessToken);

        var onDisk = File.ReadAllText(path);
        Assert.True(SecretProtector.IsProtected(onDisk));
        Assert.DoesNotContain("legacy-token", onDisk);
    }

    [Fact]
    public void HisenseFileOutsideTheConfigDirectoryIsMovedIntoItAndRemoved()
    {
        // The shape of a Windows upgrade: 5.6.0 wrote to %APPDATA%\LittleBigMouse, LbmPaths
        // says %LOCALAPPDATA%\Mgth\LittleBigMouse.
        var legacy = Path.Combine(Directory.CreateDirectory(At("legacy")).FullName, "hisense-vidaa.json");
        var path = At("hisense-vidaa.json");
        File.WriteAllText(legacy, JsonSerializer.Serialize(new Dictionary<string, HisenseVidaaConfiguration>
        {
            ["HIS123"] = new() { MonitorId = "HIS123", AccessToken = "legacy-token" },
        }));

        Assert.Equal("legacy-token", new HisenseVidaaSettingsStore(path, legacy).Get("HIS123")!.AccessToken);

        Assert.True(File.Exists(path));
        Assert.True(SecretProtector.IsProtected(File.ReadAllText(path)));
        Assert.False(File.Exists(legacy));

        // The store no longer needs the legacy path at all.
        Assert.Equal("legacy-token", new HisenseVidaaSettingsStore(path).Get("HIS123")!.AccessToken);
    }

    [Fact]
    public void HisenseLegacyFileIsIgnoredOnceTheRealOneExists()
    {
        var legacy = Path.Combine(Directory.CreateDirectory(At("legacy")).FullName, "hisense-vidaa.json");
        var path = At("hisense-vidaa.json");

        new HisenseVidaaSettingsStore(path).Save(new HisenseVidaaConfiguration
        {
            MonitorId = "HIS123", AccessToken = "current",
        });
        File.WriteAllText(legacy, JsonSerializer.Serialize(new Dictionary<string, HisenseVidaaConfiguration>
        {
            ["HIS123"] = new() { MonitorId = "HIS123", AccessToken = "stale" },
        }));

        Assert.Equal("current", new HisenseVidaaSettingsStore(path, legacy).Get("HIS123")!.AccessToken);
        Assert.True(File.Exists(legacy));
    }
}
