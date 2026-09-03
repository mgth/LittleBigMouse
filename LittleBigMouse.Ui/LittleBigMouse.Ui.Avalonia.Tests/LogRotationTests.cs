using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The startup log keeps the last runs, not just the last one. The run that matters is the
/// one that failed, and the natural reaction to an app that does not come up is to relaunch
/// it — twice was enough to lose the log of the hot-plug failure in #589.
/// </summary>
public sealed class LogRotationTests : IDisposable
{
    readonly string _dir = Path.Combine(Path.GetTempPath(), "lbm-log-rotation-tests", Guid.NewGuid().ToString("N"));

    string Log => Path.Combine(_dir, "ui.log");

    public LogRotationTests() => Directory.CreateDirectory(_dir);

    public void Dispose()
    {
        try { Directory.Delete(_dir, true); } catch { }
    }

    /// <summary>One run of the app: rotate what is there, then write this run's log.</summary>
    void Run(string content)
    {
        LogRotation.Rotate(Log);
        File.WriteAllText(Log, content);
    }

    string Previous(int generation) => File.ReadAllText(LogRotation.PreviousPath(Log, generation));

    [Fact]
    public void FirstRun_HasNothingToRotate()
    {
        LogRotation.Rotate(Log);

        Assert.Empty(Directory.GetFiles(_dir));
    }

    [Fact]
    public void GenerationsAreNamedPrevThenNumbered()
    {
        // ui.prev.log keeps its historical name: it is what every issue reply asks for.
        Assert.Equal(Path.Combine(_dir, "ui.prev.log"), LogRotation.PreviousPath(Log, 1));
        Assert.Equal(Path.Combine(_dir, "ui.prev.2.log"), LogRotation.PreviousPath(Log, 2));
        Assert.Equal(Path.Combine(_dir, "ui.prev.5.log"), LogRotation.PreviousPath(Log, 5));
    }

    [Fact]
    public void TheLastRunBecomesPrev()
    {
        Run("run 1");
        Run("run 2");

        Assert.Equal("run 2", File.ReadAllText(Log));
        Assert.Equal("run 1", Previous(1));
        Assert.False(File.Exists(LogRotation.PreviousPath(Log, 2)));
    }

    [Fact]
    public void TwoRelaunchesKeepTheRunThatMattered()
    {
        // #589: the hot-plug failure, then two boots that failed the same way.
        Run("hot-plug failure");
        Run("boot failed");
        Run("boot failed again");

        Assert.Equal("hot-plug failure", Previous(2));
    }

    [Fact]
    public void KeepsFiveRuns_TheOldestFallsOff()
    {
        for (var i = 1; i <= 7; i++) Run($"run {i}");

        Assert.Equal("run 7", File.ReadAllText(Log));
        for (var generation = 1; generation <= LogRotation.Keep; generation++)
            Assert.Equal($"run {7 - generation}", Previous(generation));
        Assert.False(File.Exists(LogRotation.PreviousPath(Log, LogRotation.Keep + 1)));
        Assert.Equal(LogRotation.Keep + 1, Directory.GetFiles(_dir).Length);
    }

    [Fact]
    public void AnInterruptedRotationIsFoldedIn()
    {
        // A crash between the two moves leaves the last run in the staging file. It must
        // enter the chain at the next start, not sit there forever.
        File.WriteAllText(Path.Combine(_dir, "ui.rotating.log"), "interrupted");
        File.WriteAllText(Log, "current");

        LogRotation.Rotate(Log);

        Assert.Equal("current", Previous(1));
        Assert.Equal("interrupted", Previous(2));
        Assert.False(File.Exists(Path.Combine(_dir, "ui.rotating.log")));
    }

    [Fact]
    public void ACurrentLogHeldOpen_LeavesTheChainUntouched()
    {
        // Windows refuses to move a file another handle holds without FileShare.Delete —
        // how a second launch finds ui.log, written by the running instance with
        // FileShare.Read. POSIX renames open files freely: nothing to prove there.
        if (!OperatingSystem.IsWindows()) return;

        Run("run 1");
        Run("run 2");

        using (new FileStream(Log, FileMode.Open, FileAccess.Write, FileShare.Read))
        {
            Assert.ThrowsAny<IOException>(() => LogRotation.Rotate(Log));

            // Still in place, and the chain behind it did not move. (ui.log itself is only
            // readable once the handle is gone: the holder's write access excludes a reader.)
            Assert.True(File.Exists(Log));
            Assert.Equal("run 1", Previous(1));
            Assert.False(File.Exists(LogRotation.PreviousPath(Log, 2)));
        }

        Assert.Equal("run 2", File.ReadAllText(Log));
    }
}
