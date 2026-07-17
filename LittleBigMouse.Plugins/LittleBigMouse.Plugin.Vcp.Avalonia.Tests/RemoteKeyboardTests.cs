#nullable enable
using Avalonia.Input;
using LittleBigMouse.Plugin.Vcp.Avalonia;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class RemoteKeyboardTests
{
    [Theory]
    [InlineData(Key.Up, "KEY_UP", "KEY_UP")]
    [InlineData(Key.Home, "KEY_HOME", "KEY_HOME")]
    [InlineData(Key.VolumeMute, "KEY_MUTE", "KEY_MUTE")]
    [InlineData(Key.NumPad7, "KEY_7", "KEY_7")]
    [InlineData(Key.Enter, "KEY_ENTER", "KEY_OK")]
    [InlineData(Key.Escape, "KEY_RETURN", "KEY_BACK")]
    public void MapsSharedKeyboardInputToEachRemoteProtocol(
        Key key,
        string expectedSamsung,
        string expectedHisense)
    {
        Assert.Equal(expectedSamsung, RemoteKeyboardProfile.Samsung.RemoteKeyFor(key));
        Assert.Equal(expectedHisense, RemoteKeyboardProfile.Hisense.RemoteKeyFor(key));
    }

    [Fact]
    public void LeavesUnrelatedKeyboardInputUnhandledForBothRemotes()
    {
        Assert.Null(RemoteKeyboardProfile.Samsung.RemoteKeyFor(Key.Tab));
        Assert.Null(RemoteKeyboardProfile.Hisense.RemoteKeyFor(Key.Tab));
    }
}
