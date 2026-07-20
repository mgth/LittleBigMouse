using System.Security.Cryptography;
using System.Security.Cryptography.X509Certificates;
using System.Text.Json;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public sealed class SmartTvSecurityTests
{
    [Fact]
    public void PairingCapturesCertificateAndNormalConnectionsRequireTheSamePin()
    {
        using var trusted = CreateCertificate("CN=trusted-tv");
        using var impostor = CreateCertificate("CN=impostor-tv");
        var pairing = new DeviceCertificatePin(null, allowNewFingerprint: true);

        Assert.True(pairing.Validate(this, trusted, null, default));
        Assert.Equal(64, pairing.ObservedFingerprint.Length);

        var pinned = new DeviceCertificatePin(pairing.ObservedFingerprint,
            allowNewFingerprint: false);
        Assert.True(pinned.Validate(this, trusted, null, default));
        Assert.False(pinned.Validate(this, impostor, null, default));
        Assert.False(new DeviceCertificatePin(null, false)
            .Validate(this, trusted, null, default));
    }

    [Fact]
    public void SecretProtectorRoundTripsWithoutPersistingPlaintext()
    {
        using var directory = new TemporaryDirectory();
        const string plaintext = "unique-smart-tv-secret-845792";
        var protector = new SmartTvSecretProtector(directory.Path);

        var protectedValue = protector.Protect(plaintext);

        Assert.DoesNotContain(plaintext, protectedValue, StringComparison.Ordinal);
        Assert.Equal(plaintext, protector.Unprotect(protectedValue));
    }

    [Fact]
    public void SamsungStoreEncryptsTokenAndMigratesLegacyPlaintextOnSave()
    {
        using var directory = new TemporaryDirectory();
        var path = System.IO.Path.Combine(directory.Path, "samsung.json");
        const string token = "unique-samsung-token-178265";
        File.WriteAllText(path, JsonSerializer.Serialize(new Dictionary<string, SamsungTizenConfiguration>
        {
            ["SAM123"] = new()
            {
                MonitorId = "SAM123",
                IpAddress = "192.168.1.42",
                Token = token,
                ServerCertificateFingerprint = new string('A', 64),
            },
        }));

        var store = new SamsungTizenSettingsStore(path);
        var legacy = Assert.IsType<SamsungTizenConfiguration>(store.Get("SAM123"));
        Assert.Equal(token, legacy.Token);
        store.Save(legacy);

        Assert.DoesNotContain(token, File.ReadAllText(path), StringComparison.Ordinal);
        var reloaded = Assert.IsType<SamsungTizenConfiguration>(
            new SamsungTizenSettingsStore(path).Get("SAM123"));
        Assert.Equal(token, reloaded.Token);
        Assert.Equal(new string('A', 64), reloaded.ServerCertificateFingerprint);
    }

    [Fact]
    public void VidaaStoreEncryptsAllReusableCredentials()
    {
        using var directory = new TemporaryDirectory();
        var path = System.IO.Path.Combine(directory.Path, "vidaa.json");
        var configuration = new HisenseVidaaConfiguration
        {
            MonitorId = "HIS123",
            IpAddress = "192.168.1.43",
            ClientCertificatePassword = "unique-cert-password-28391",
            ClientId = "unique-client-id-73419",
            MqttUsername = "unique-mqtt-user-94271",
            AccessToken = "unique-access-token-47291",
            RefreshToken = "unique-refresh-token-62915",
            ServerCertificateFingerprint = new string('B', 64),
        };

        new HisenseVidaaSettingsStore(path).Save(configuration);
        var json = File.ReadAllText(path);
        Assert.DoesNotContain(configuration.ClientCertificatePassword, json, StringComparison.Ordinal);
        Assert.DoesNotContain(configuration.ClientId, json, StringComparison.Ordinal);
        Assert.DoesNotContain(configuration.MqttUsername, json, StringComparison.Ordinal);
        Assert.DoesNotContain(configuration.AccessToken, json, StringComparison.Ordinal);
        Assert.DoesNotContain(configuration.RefreshToken, json, StringComparison.Ordinal);

        var loaded = Assert.IsType<HisenseVidaaConfiguration>(
            new HisenseVidaaSettingsStore(path).Get("HIS123"));
        Assert.Equal(configuration.ClientCertificatePassword, loaded.ClientCertificatePassword);
        Assert.Equal(configuration.ClientId, loaded.ClientId);
        Assert.Equal(configuration.MqttUsername, loaded.MqttUsername);
        Assert.Equal(configuration.AccessToken, loaded.AccessToken);
        Assert.Equal(configuration.RefreshToken, loaded.RefreshToken);
        Assert.Equal(configuration.ServerCertificateFingerprint,
            loaded.ServerCertificateFingerprint);
    }

    static X509Certificate2 CreateCertificate(string subject)
    {
        using var key = RSA.Create(2048);
        var request = new CertificateRequest(subject, key, HashAlgorithmName.SHA256,
            RSASignaturePadding.Pkcs1);
        return request.CreateSelfSigned(DateTimeOffset.UtcNow.AddMinutes(-1),
            DateTimeOffset.UtcNow.AddDays(1));
    }

    sealed class TemporaryDirectory : IDisposable
    {
        public TemporaryDirectory()
        {
            Path = System.IO.Path.Combine(System.IO.Path.GetTempPath(),
                "littlebigmouse-tests", Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(Path);
        }

        public string Path { get; }

        public void Dispose()
        {
            if (Directory.Exists(Path)) Directory.Delete(Path, recursive: true);
        }
    }
}
