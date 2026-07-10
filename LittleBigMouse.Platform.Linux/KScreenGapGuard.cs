#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Works around the InputCapture barrier validator: both KWin (xdg-desktop-portal-kde,
/// checkAndMakeBarrier) and mutter reject any barrier on an edge shared between two
/// contiguous outputs — the portal spec mandates the outside boundary of the zone union,
/// because it was designed for the inter-machine case (Synergy) where every useful edge
/// is an outer one. LBM's whole point is intercepting *interior* crossings, so while the
/// engine runs we shift outputs apart by one logical pixel: every shared edge becomes an
/// outer edge, the daemon's full-edge barriers pass validation, and LBM is the sole
/// router of monitor crossings — exactly the Windows semantic.
///
/// The original positions are journaled to disk BEFORE touching the compositor, so a
/// crash anywhere (UI, daemon, compositor) is recovered by <see cref="RecoverStale"/> on
/// the next start. An output the user moved while gapped (systemsettings, kscreen) is
/// left alone on restore.
/// </summary>
public static class KScreenGapGuard
{
    static string StateFile => Path.Combine(LbmPaths.DataDir, "kscreen-restore.json");

    // Gaps only matter for the Wayland portal backend: under X11 the daemon emulates
    // ClipCursor directly (no barriers), and without kscreen there is nothing to drive.
    static bool IsWaylandKde
        => Environment.GetEnvironmentVariable("XDG_CURRENT_DESKTOP")?.Contains("KDE", StringComparison.OrdinalIgnoreCase) == true
           && (Environment.GetEnvironmentVariable("XDG_SESSION_TYPE") == "wayland"
               || !string.IsNullOrEmpty(Environment.GetEnvironmentVariable("WAYLAND_DISPLAY")));

    record GapEntry(string Name, int OriginalX, int OriginalY, int AppliedX, int AppliedY);

    /// <summary>
    /// Open a 1px logical gap at every shared edge between enabled outputs.
    /// Idempotent: an already-gapped topology computes zero shifts and is a no-op.
    /// Returns true when the topology actually changed (caller should raise DisplayChanged).
    /// </summary>
    public static bool Apply(IReadOnlyList<LinuxMonitor> monitors)
    {
        if (!IsWaylandKde) return false;

        var enabled = monitors.Where(m => m.Enabled).ToList();
        var shifts = ComputeShifts(enabled);
        if (shifts.Count == 0) return false;

        // Journal first. Merge with an existing journal (engine restarted while gapped,
        // or an output was plugged in mid-run): entries already present keep their
        // original positions — the live geometry is gapped, not original.
        var journal = LoadState().ToDictionary(e => e.Name);
        foreach (var m in enabled)
        {
            if (!shifts.TryGetValue(m.ConnectorName, out var shift)) continue;
            var x = (int)Math.Round(m.LogicalX);
            var y = (int)Math.Round(m.LogicalY);
            journal[m.ConnectorName] = journal.TryGetValue(m.ConnectorName, out var existing)
                ? existing with { AppliedX = x + shift.Dx, AppliedY = y + shift.Dy }
                : new GapEntry(m.ConnectorName, x, y, x + shift.Dx, y + shift.Dy);
        }
        SaveState(journal.Values);

        var args = string.Join(' ', enabled
            .Where(m => shifts.ContainsKey(m.ConnectorName))
            .Select(m => $"output.{m.ConnectorName}.position.{(int)Math.Round(m.LogicalX) + shifts[m.ConnectorName].Dx},{(int)Math.Round(m.LogicalY) + shifts[m.ConnectorName].Dy}"));

        return LinuxDisplayController.Run("kscreen-doctor", args);
    }

    /// <summary>
    /// Put outputs back where they were before <see cref="Apply"/>. Outputs whose live
    /// position no longer matches what we applied were moved by the user in the meantime
    /// and are left untouched. The journal is deleted once nothing is left to restore.
    /// Returns true when the topology actually changed.
    /// </summary>
    public static bool Restore(IReadOnlyList<LinuxMonitor> monitors)
    {
        var journal = LoadState();
        if (journal.Count == 0) return false;

        var live = monitors.ToDictionary(m => m.ConnectorName);
        var moves = new List<string>();
        foreach (var entry in journal)
        {
            if (!live.TryGetValue(entry.Name, out var m)) continue; // unplugged: nothing to do
            var x = (int)Math.Round(m.LogicalX);
            var y = (int)Math.Round(m.LogicalY);
            if (x == entry.OriginalX && y == entry.OriginalY) continue;      // already back
            if (x != entry.AppliedX || y != entry.AppliedY) continue;        // user moved it
            moves.Add($"output.{entry.Name}.position.{entry.OriginalX},{entry.OriginalY}");
        }

        if (moves.Count == 0)
        {
            DeleteState();
            return false;
        }

        if (!LinuxDisplayController.Run("kscreen-doctor", string.Join(' ', moves)))
            return false; // keep the journal, a later Restore/RecoverStale retries

        DeleteState();
        return true;
    }

    /// <summary>
    /// Crash recovery, called once at startup: a leftover journal means a previous run
    /// died while the topology was gapped — restore it before building the first layout.
    /// </summary>
    public static bool RecoverStale(IReadOnlyList<LinuxMonitor> monitors)
        => File.Exists(StateFile) && Restore(monitors);

    /// <summary>
    /// One shift per output needing to move: for every vertical boundary where two
    /// outputs touch (A.right == B.left with y-overlap), every output at or beyond that
    /// boundary moves +1 in x — cumulative, so relative alignment elsewhere is preserved
    /// and each shared edge opens by exactly one pixel. Same on the y axis for stacks.
    /// </summary>
    static Dictionary<string, (int Dx, int Dy)> ComputeShifts(List<LinuxMonitor> monitors)
    {
        var xCuts = new SortedSet<int>();
        var yCuts = new SortedSet<int>();

        foreach (var a in monitors)
        foreach (var b in monitors)
        {
            if (ReferenceEquals(a, b)) continue;
            var (ax, ay) = ((int)Math.Round(a.LogicalX), (int)Math.Round(a.LogicalY));
            var (bx, by) = ((int)Math.Round(b.LogicalX), (int)Math.Round(b.LogicalY));
            var (aw, ah) = ((int)Math.Round(a.LogicalWidth), (int)Math.Round(a.LogicalHeight));
            var (bw, bh) = ((int)Math.Round(b.LogicalWidth), (int)Math.Round(b.LogicalHeight));

            if (ax + aw == bx && Overlaps(ay, ah, by, bh)) xCuts.Add(bx);
            if (ay + ah == by && Overlaps(ax, aw, bx, bw)) yCuts.Add(by);
        }

        var shifts = new Dictionary<string, (int Dx, int Dy)>();
        foreach (var m in monitors)
        {
            var dx = xCuts.Count(c => c <= (int)Math.Round(m.LogicalX));
            var dy = yCuts.Count(c => c <= (int)Math.Round(m.LogicalY));
            if (dx != 0 || dy != 0) shifts[m.ConnectorName] = (dx, dy);
        }
        return shifts;
    }

    static bool Overlaps(int start1, int length1, int start2, int length2)
        => Math.Min(start1 + length1, start2 + length2) - Math.Max(start1, start2) > 0;

    static List<GapEntry> LoadState()
    {
        try
        {
            if (!File.Exists(StateFile)) return [];
            return JsonSerializer.Deserialize<List<GapEntry>>(File.ReadAllText(StateFile)) ?? [];
        }
        catch
        {
            return [];
        }
    }

    static void SaveState(IEnumerable<GapEntry> entries)
    {
        Directory.CreateDirectory(LbmPaths.DataDir);
        // Atomic like LinuxLayoutPersistence: the journal is the crash-safety net, a torn
        // write would leave the user's topology unrecoverable.
        var tmp = StateFile + ".tmp";
        File.WriteAllText(tmp, JsonSerializer.Serialize(entries.ToList(), new JsonSerializerOptions { WriteIndented = true }));
        File.Move(tmp, StateFile, overwrite: true);
    }

    static void DeleteState()
    {
        try { File.Delete(StateFile); }
        catch { /* a stale journal only costs a harmless RecoverStale later */ }
    }
}
