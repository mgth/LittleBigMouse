using System;
using System.Collections.Generic;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using System.Xml;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// What gets written to the crash-recovery / autostart file, and when. The daemon replays this
/// file at boot with no UI reachable, so the decision of "remember this / don't / strip the Run
/// line" is the whole safety contract — a client's foreign geometry replayed over the local
/// desktop, or a stopped layout that re-hooks the mouse, both start here.
/// </summary>
public sealed class RecoveryStateStoreTests
{
    /// <summary>A recovery writer that records instead of touching disk, and can be told to fail.</summary>
    sealed class FakeWriter : IRecoveryFileWriter
    {
        public string? WrittenPath { get; private set; }
        public string? WrittenContent { get; private set; }
        public int Writes { get; private set; }
        public int MarkStoppedCalls { get; private set; }

        public Exception? WriteThrows { get; set; }
        public Exception? MarkStoppedThrows { get; set; }

        public Task WriteAsync(string path, string content, CancellationToken token)
        {
            if (WriteThrows is not null) throw WriteThrows;
            Writes++;
            WrittenPath = path;
            WrittenContent = content;
            return Task.CompletedTask;
        }

        public Task MarkStoppedAsync(string path, CancellationToken token)
        {
            if (MarkStoppedThrows is not null) throw MarkStoppedThrows;
            MarkStoppedCalls++;
            return Task.CompletedTask;
        }
    }

    const string Path = "/fake/Current.xml";

    static RecoveryStateStore Store(FakeWriter writer) => new(writer, Path);

    static CommandMessage Load(bool virtualLayout = false)
        => new(LittleBigMouseCommand.Load, new ZonesLayout { Virtual = virtualLayout });

    static CommandMessage Run() => new(LittleBigMouseCommand.Run);
    static CommandMessage Stop() => new(LittleBigMouseCommand.Stop, (ZonesLayout?)null);

    [Fact]
    public async Task ARealLoadIsWrittenToTheRecoveryFile()
    {
        var writer = new FakeWriter();

        var failure = await Store(writer).PersistAsync([Load(), Run()], persist: true);

        Assert.Null(failure);
        Assert.Equal(1, writer.Writes);
        Assert.Equal(Path, writer.WrittenPath);
        // Both lines land, newline-terminated: the daemon reads it back line by line.
        Assert.Contains("Command=\"Load\"", writer.WrittenContent);
        Assert.Contains("Command=\"Run\"", writer.WrittenContent);
        Assert.EndsWith("\n", writer.WrittenContent);
    }

    [Fact]
    public async Task AVirtualLoadIsNeverPersisted()
    {
        // A standalone daemon must not replay a client's geometry over the local desktop.
        var writer = new FakeWriter();

        var failure = await Store(writer).PersistAsync([Load(virtualLayout: true), Run()], persist: true);

        Assert.Null(failure);
        Assert.Equal(0, writer.Writes);
        Assert.Equal(0, writer.MarkStoppedCalls);
    }

    [Fact]
    public async Task ANonPersistingSendWritesNothing()
    {
        // Live preview: the recovery file keeps describing the last applied layout.
        var writer = new FakeWriter();

        var failure = await Store(writer).PersistAsync([Load(), Run()], persist: false);

        Assert.Null(failure);
        Assert.Equal(0, writer.Writes);
    }

    [Fact]
    public async Task AStopStripsTheRunLineFromTheExistingFile()
    {
        var writer = new FakeWriter();

        var failure = await Store(writer).PersistAsync([Stop()], persist: true);

        Assert.Null(failure);
        Assert.Equal(0, writer.Writes);
        Assert.Equal(1, writer.MarkStoppedCalls);
    }

    [Fact]
    public async Task AFailedLoadWriteIsReturnedSoTheCallerCanSurfaceIt()
    {
        // The live layout was applied but recovery is stale: the caller turns this into the
        // "settings could not be saved" warning.
        var writer = new FakeWriter { WriteThrows = new IOException("disk full") };

        var failure = await Store(writer).PersistAsync([Load(), Run()], persist: true);

        Assert.IsType<IOException>(failure);
    }

    [Fact]
    public async Task AnXmlFailureOnLoadIsAlsoReturned()
    {
        var writer = new FakeWriter { WriteThrows = new XmlException("bad") };

        var failure = await Store(writer).PersistAsync([Load(), Run()], persist: true);

        Assert.IsType<XmlException>(failure);
    }

    [Fact]
    public async Task AFailedStopIsSwallowedNeverReturned()
    {
        // Stop is a safety operation: a persistence failure must never fault it.
        var writer = new FakeWriter { MarkStoppedThrows = new UnauthorizedAccessException() };

        var failure = await Store(writer).PersistAsync([Stop()], persist: true);

        Assert.Null(failure);
    }

    [Fact]
    public async Task ALoadTakesPrecedenceOverAStopInTheSameBatch()
    {
        // A batch with both a real Load and a Stop is a Start-shaped send: write, don't strip.
        var writer = new FakeWriter();

        var failure = await Store(writer).PersistAsync([Load(), Stop()], persist: true);

        Assert.Null(failure);
        Assert.Equal(1, writer.Writes);
        Assert.Equal(0, writer.MarkStoppedCalls);
    }
}
