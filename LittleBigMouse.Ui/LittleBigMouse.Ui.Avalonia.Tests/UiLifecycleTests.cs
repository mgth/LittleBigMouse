using MsBox.Avalonia.Enums;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
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

    sealed class TestResource : IDisposable
    {
        public bool Disposed { get; private set; }
        public void Dispose() => Disposed = true;
    }
}
