using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Xml;
using LittleBigMouse.Plugins;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Owns the crash-recovery / autostart file (<c>Current.xml</c>) that a standalone daemon
/// reads back at boot. This is the persistence half of a send, pulled out of
/// <see cref="LittleBigMouseClientService"/> so the file format, path and "what counts as a
/// change worth remembering" decision live in one place.
/// <para>
/// The daemon reads this file back on startup: the path must match its side
/// (<c>%LOCALAPPDATA%\Mgth\LittleBigMouse</c> on Windows, <c>~/.local/share/LittleBigMouse</c>
/// on Linux — <see cref="LbmPaths.DataDir"/>). The former literal
/// <c>@"Mgth\LittleBigMouse\Current.xml"</c> produced a file NAMED with backslashes on Linux.
/// </para>
/// <para>
/// The two file operations are seams (<see cref="IRecoveryFileWriter"/>) so tests can drive the
/// persistence decisions without touching a real disk; production uses
/// <see cref="AtomicRecoveryFileWriter"/>, a thin pass-through to <see cref="AtomicRecoveryFile"/>.
/// </para>
/// </summary>
public sealed class RecoveryStateStore
{
    readonly IRecoveryFileWriter _writer;
    readonly string _path;

    public RecoveryStateStore() : this(new AtomicRecoveryFileWriter(),
        Path.Combine(LbmPaths.DataDir, "Current.xml"))
    {
    }

    internal RecoveryStateStore(IRecoveryFileWriter writer, string path)
    {
        _writer = writer;
        _path = path;
    }

    /// <summary>
    /// Fold the crash-recovery consequence of a batch of commands into the file, then report
    /// whether it succeeded. A Load of a real layout is written verbatim (one command per line);
    /// a Stop strips the Run line from what is already there so the daemon replays stopped.
    /// Nothing else touches the file.
    /// </summary>
    /// <param name="commands">The batch about to be sent to the daemon.</param>
    /// <param name="persist">
    /// False for a send the daemon should run but not remember (live preview): the recovery file
    /// keeps describing the last applied layout. Serializing a layout is the expensive half of a
    /// send, so the recovery copy is only built when it is actually going to be written.
    /// </param>
    /// <returns>
    /// Null when nothing was persisted or a Load was persisted successfully. A non-null exception
    /// when a Load could not be written — the live configuration was applied but recovery is stale,
    /// which the caller surfaces. A failed Stop is swallowed here (Stop is a safety operation) and
    /// never returned.
    /// </returns>
    public async Task<Exception?> PersistAsync(IReadOnlyList<CommandMessage> commands,
        bool persist, CancellationToken token = default)
    {
        if (!persist) return null;

        // Virtual (foreign) layouts are sent to the daemon for inspection but must never become
        // the crash-recovery/autostart state: a standalone daemon would replay a client's geometry
        // over the local desktop at the next boot.
        if (commands.Any(command => command.Command == LittleBigMouseCommand.Load
                                    && command.Payload?.Virtual != true))
        {
            var recoveryXml = string.Join("\n", commands.Select(command => command.Serialize())) + "\n";
            try
            {
                await _writer.WriteAsync(_path, recoveryXml, token);
                return null;
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException or XmlException)
            {
                return error;
            }
        }

        if (commands.Any(command => command.Command == LittleBigMouseCommand.Stop))
        {
            // A user Stop must survive a restart: strip the Run line so a standalone daemon
            // replays the layout stopped instead of re-hooking the mouse at the next boot. Stop is
            // a safety operation — a persistence failure must never fault it.
            try
            {
                await _writer.MarkStoppedAsync(_path, token);
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException or XmlException)
            {
                System.Diagnostics.Debug.WriteLine($"Could not persist the stopped state: {error.Message}");
            }
        }

        return null;
    }
}

/// <summary>
/// The two file operations <see cref="RecoveryStateStore"/> needs, as a seam for tests. The
/// production implementation is <see cref="AtomicRecoveryFileWriter"/>.
/// </summary>
public interface IRecoveryFileWriter
{
    Task WriteAsync(string path, string content, CancellationToken token);
    Task MarkStoppedAsync(string path, CancellationToken token);
}

/// <summary>Production writer: a thin pass-through to <see cref="AtomicRecoveryFile"/>.</summary>
public sealed class AtomicRecoveryFileWriter : IRecoveryFileWriter
{
    public Task WriteAsync(string path, string content, CancellationToken token)
        => AtomicRecoveryFile.WriteAsync(path, content, token);

    public Task MarkStoppedAsync(string path, CancellationToken token)
        => AtomicRecoveryFile.MarkStoppedAsync(path, token);
}
