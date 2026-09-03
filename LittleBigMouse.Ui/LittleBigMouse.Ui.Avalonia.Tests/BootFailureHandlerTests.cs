using LittleBigMouse.Ui.Avalonia.Main;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// A failed boot must end the process, not park it: the parked process had no window, no
/// tray icon, and held the single-instance lock, so every relaunch exited silently (#589).
/// The handler's three steps are log, show, shut down — and the last one has to happen no
/// matter what the two before it did.
/// </summary>
public sealed class BootFailureHandlerTests
{
    static readonly Exception Boom =
        new ArgumentException("Registry key names should not be greater than 255 characters.");

    sealed class Recorder
    {
        public readonly List<string> Log = [];
        public readonly List<string> Steps = [];
        public Exception? Shown;
        public int LogLinesWhenShown = -1;
        public bool DialogThrows;
        public bool ShutdownThrows;

        public BootFailureHandler Handler => new(
            log: Log.Add,
            showAsync: error =>
            {
                Shown = error;
                LogLinesWhenShown = Log.Count;
                Steps.Add("show");
                return DialogThrows
                    ? Task.FromException(new InvalidOperationException("no dispatcher"))
                    : Task.CompletedTask;
            },
            shutdown: () =>
            {
                Steps.Add("shutdown");
                if (ShutdownThrows) throw new InvalidOperationException("already shut down");
            });
    }

    [Fact]
    public async Task LogsThenShowsThenShutsDown()
    {
        var recorder = new Recorder();

        await recorder.Handler.HandleAsync(Boom);

        Assert.Equal(["show", "shutdown"], recorder.Steps);
        Assert.Same(Boom, recorder.Shown);

        // The log line is the one thing that survives the dialog being closed unread:
        // written first, with the exception in full.
        var line = Assert.Single(recorder.Log);
        Assert.StartsWith("Boot failed: ", line);
        Assert.Contains(Boom.Message, line);
        Assert.Equal(1, recorder.LogLinesWhenShown);
    }

    [Fact]
    public async Task ShutsDownWhenTheDialogCannotBeShown()
    {
        var recorder = new Recorder { DialogThrows = true };

        await recorder.Handler.HandleAsync(Boom);

        Assert.Equal("shutdown", recorder.Steps.Last());
        Assert.Contains(recorder.Log, l => l.StartsWith("Boot failure could not be shown: "));
    }

    [Fact]
    public async Task AShutdownFailureIsLoggedNotThrown()
    {
        var recorder = new Recorder { ShutdownThrows = true };

        await recorder.Handler.HandleAsync(Boom);

        Assert.Contains(recorder.Log, l => l.StartsWith("Shutdown after boot failure failed: "));
    }
}
