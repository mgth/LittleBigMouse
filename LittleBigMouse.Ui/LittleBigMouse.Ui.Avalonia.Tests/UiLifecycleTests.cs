using MsBox.Avalonia.Enums;
using LittleBigMouse.Platform.Windows;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using LittleBigMouse.Ui.Core;
using System.Security.Cryptography;
using System.Text.Json.Nodes;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class UiLifecycleTests
{
    [Theory]
    [InlineData(ButtonResult.Yes, true)]
    [InlineData(ButtonResult.No, false)]
    [InlineData(ButtonResult.None, false)]
    public void CloseConfirmationOnlyAcceptsYes(ButtonResult result, bool expected)
        => Assert.Equal(expected, MainViewModel.ShouldShutdown(result));

    [Fact]
    public void OlderAsyncResourceCannotReplaceNewerOne()
    {
        using var slot = new LatestResourceSlot<TestResource>();
        var olderGeneration = slot.Begin();
        var newerGeneration = slot.Begin();
        var newer = new TestResource();
        var older = new TestResource();

        Assert.True(slot.TryReplace(newerGeneration, newer));
        Assert.False(slot.TryReplace(olderGeneration, older));
        Assert.True(older.Disposed);
        Assert.False(newer.Disposed);
    }

    [Fact]
    public void ReplacingAndDisposingResourceReleasesEveryOwnedInstance()
    {
        var slot = new LatestResourceSlot<TestResource>();
        var first = new TestResource();
        var second = new TestResource();
        Assert.True(slot.TryReplace(slot.Begin(), first));
        Assert.True(slot.TryReplace(slot.Begin(), second));

        Assert.True(first.Disposed);
        Assert.False(second.Disposed);
        slot.Dispose();
        Assert.True(second.Disposed);
    }

    [Fact]
    public void ReleaseSelectionIgnoresUnexpectedAssetsAndRequiresDigest()
    {
        var releases = JsonNode.Parse("""
            [{
              "tag_name":"v6.0.0", "draft":false, "prerelease":false, "body":"safe",
              "assets":[
                {"name":"notes.exe","state":"uploaded","size":7,"digest":"sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","browser_download_url":"https://github.com/mgth/LittleBigMouse/releases/download/v6.0.0/notes.exe"},
                {"name":"LittleBigMouse_6.0.0.exe","state":"uploaded","size":42,"digest":"sha256:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB","browser_download_url":"https://github.com/mgth/LittleBigMouse/releases/download/v6.0.0/LittleBigMouse_6.0.0.exe"}
              ]
            }]
            """);

        var selected = Assert.IsType<ReleaseUpdateInfo>(
            ReleaseUpdateSecurity.SelectNewest(releases));
        Assert.Equal("LittleBigMouse_6.0.0.exe", selected.FileName);
        Assert.Equal(new string('B', 64), selected.Sha256);

        releases![0]!["assets"]![1]!["digest"] = null;
        Assert.Null(ReleaseUpdateSecurity.SelectNewest(releases));
    }

    [Fact]
    public void ReleaseDigestVerificationRejectsChangedBytes()
    {
        var path = Path.GetTempFileName();
        try
        {
            File.WriteAllText(path, "trusted bytes");
            var digest = Convert.ToHexString(SHA256.HashData(File.ReadAllBytes(path)));
            Assert.True(ReleaseUpdateSecurity.VerifySha256(path, digest));
            File.AppendAllText(path, " changed");
            Assert.False(ReleaseUpdateSecurity.VerifySha256(path, digest));
        }
        finally { File.Delete(path); }
    }

    [Fact]
    public void MalformedReleaseMetadataIsSkippedWithoutHidingAValidRelease()
    {
        var releases = JsonNode.Parse("""
            [
              {"tag_name":{"unexpected":true},"draft":"not-a-boolean","assets":[]},
              {
                "tag_name":"v6.1.0","draft":false,"prerelease":false,
                "assets":[{
                  "name":"LittleBigMouse_6.1.0.exe","state":"uploaded","size":42,
                  "digest":"sha256:CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC",
                  "browser_download_url":"https://github.com/mgth/LittleBigMouse/releases/download/v6.1.0/LittleBigMouse_6.1.0.exe"
                }]
              }
            ]
            """);

        Assert.Equal(new Version(6, 1, 0),
            ReleaseUpdateSecurity.SelectNewest(releases)?.Version);
    }

    [Fact]
    public void UnsignedExecutableIsNeverAcceptedAsAnUpdate()
        => Assert.False(AuthenticodeVerifier.IsTrustedPublisher(
            typeof(UiLifecycleTests).Assembly.Location,
            ReleaseUpdateSecurity.ExpectedPublisher));

    [Fact]
    public void ElevationPolicyNeverElevatesTheUi()
    {
        Assert.Equal(Microsoft.Win32.TaskScheduler.TaskRunLevel.LUA,
            AutostartExtensions.ScheduledUiRunLevel(elevateHook: true));
        var normal = DaemonLaunchPolicy.Create("hook.exe", elevate: false);
        Assert.False(normal.UseShellExecute);
        Assert.Empty(normal.Verb);
        var elevated = DaemonLaunchPolicy.Create("hook.exe", elevate: true);
        Assert.True(elevated.UseShellExecute);
        Assert.Equal("runas", elevated.Verb);
    }

    [Fact]
    public async Task StartupUpdateFailureIsContained()
    {
        var updater = new FailingUpdater();
        await MainBootloader.CheckUpdateSafelyAsync(updater);
        Assert.True(updater.Called);
    }

    sealed class TestResource : IDisposable
    {
        public bool Disposed { get; private set; }
        public void Dispose() => Disposed = true;
    }

    sealed class FailingUpdater : IApplicationUpdater
    {
        public bool Called { get; private set; }
        public Task CheckUpdateAsync(bool show)
        {
            Called = true;
            throw new HttpRequestException("offline");
        }
    }
}
