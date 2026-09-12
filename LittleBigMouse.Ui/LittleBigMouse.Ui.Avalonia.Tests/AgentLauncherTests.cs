using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// Starting an agent when none answers. The rule is narrow on purpose — one start, never a
/// stop — because the two processes have different lifetimes: the window leaves when the
/// user closes it, the agent stays.
/// </summary>
public sealed class AgentLauncherTests
{
    [Fact]
    public void WithNoAgentBinaryAnywhereNothingIsStarted()
    {
        // A frontend installed without its agent: say so, once, and carry on showing a
        // dead engine — there is nothing else honest to do.
        var started = new List<string>();
        var launcher = new AgentLauncher(
            find: () => null,
            start: path => { started.Add(path); return true; });

        Assert.False(launcher.Launch());
        Assert.Empty(started);
    }

    [Fact]
    public void TheAgentThatWasFoundIsTheOneStarted()
    {
        var started = new List<string>();
        var launcher = new AgentLauncher(
            find: () => "/opt/lbm/lbm-agent",
            start: path => { started.Add(path); return true; });

        Assert.True(launcher.Launch());
        Assert.Equal(["/opt/lbm/lbm-agent"], started);
    }

    [Fact]
    public void AnAgentThatRefusesToStartIsNotReportedAsStarted()
    {
        // The caller has nothing to retry with, but it must not go on believing an agent
        // is coming: a missing library, a binary without the execute bit.
        var launcher = new AgentLauncher(find: () => "/opt/lbm/lbm-agent", start: _ => false);

        Assert.False(launcher.Launch());
    }

    [Fact]
    public void TheNameIsThePlatformsOwn()
        => Assert.Equal(
            OperatingSystem.IsWindows() ? "lbm-agent.exe" : "lbm-agent",
            AgentLauncher.ExeName);

    [Fact]
    public void LookingForAnAgentNeverThrowsWhereverThisIsRunningFrom()
    {
        // The probe walks paths derived from where this assembly sits, which under a test
        // runner is nowhere it expects. Answering "no agent" is fine; throwing into the
        // boot sequence is not.
        var exception = Record.Exception(() => AgentLauncher.Find());

        Assert.Null(exception);
    }
}
