#nullable enable
using System.IO;

namespace LittleBigMouse.Ui.Avalonia;

/// <summary>
/// Keeps the logs of the last runs: <c>ui.log</c> is the current run, <c>ui.prev.log</c> the
/// one before, then <c>ui.prev.2.log</c> … <c>ui.prev.5.log</c>, oldest last.
/// <para>
/// One previous generation was not enough. The run that matters is the one that failed,
/// and relaunching twice is the natural reaction to an app that does not come up: by the
/// time the reporter of #589 attached the logs, the run with the hot-plug failure had been
/// overwritten by two failed boots that said the same thing.
/// </para>
/// <para>
/// The current log is taken out of the way <em>first</em>: that is the move that fails while
/// another instance holds the file open (a second launch, about to exit on the single-instance
/// guard), and nothing must have shifted by then — the running instance keeps its chain intact.
/// </para>
/// </summary>
internal static class LogRotation
{
    /// <summary>How many previous runs are kept.</summary>
    public const int Keep = 5;

    /// <summary>The file holding the run that ended <paramref name="generation"/> runs ago (1 = the last one).</summary>
    public static string PreviousPath(string path, int generation)
    {
        var dir = Path.GetDirectoryName(path) ?? "";
        var stem = Path.GetFileNameWithoutExtension(path);
        var ext = Path.GetExtension(path);
        return Path.Combine(dir, generation == 1 ? $"{stem}.prev{ext}" : $"{stem}.prev.{generation}{ext}");
    }

    static string StagingPath(string path)
        => Path.Combine(Path.GetDirectoryName(path) ?? "", $"{Path.GetFileNameWithoutExtension(path)}.rotating{Path.GetExtension(path)}");

    /// <summary>
    /// Make room for a new <paramref name="path"/>: the file there becomes the first previous
    /// generation, every older one moves down, the oldest falls off. Throws when the current
    /// log cannot be moved (held open by another instance), with the chain untouched.
    /// </summary>
    public static void Rotate(string path, int keep = Keep)
    {
        var staging = StagingPath(path);

        // A rotation interrupted between the two moves below left the last run in staging:
        // fold it in rather than leave it orphaned forever.
        if (File.Exists(staging)) Archive(path, staging, keep);

        if (!File.Exists(path)) return;

        File.Move(path, staging);
        Archive(path, staging, keep);
    }

    /// <summary>Shift the chain from the oldest, then file <paramref name="staged"/> as the last run.</summary>
    static void Archive(string path, string staged, int keep)
    {
        for (var generation = keep - 1; generation >= 1; generation--)
        {
            var from = PreviousPath(path, generation);
            if (File.Exists(from)) File.Move(from, PreviousPath(path, generation + 1), overwrite: true);
        }

        File.Move(staged, PreviousPath(path, 1), overwrite: true);
    }
}
