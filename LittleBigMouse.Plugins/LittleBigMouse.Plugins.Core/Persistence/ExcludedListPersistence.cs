#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// The excluded-processes list: its file, its default entries and the one-time top-up
/// that brings new defaults to existing installations. It sits beside
/// <see cref="ILayoutStore"/> rather than in it, because the list is a plain-text FILE the
/// daemon reads directly — the store only holds the version counter that keeps the top-up
/// one-time.
/// <para>
/// Every write is best effort: a read-only or missing directory must never keep the app
/// from starting, and the in-memory list is correct either way.
/// </para>
/// </summary>
public sealed class ExcludedListPersistence(ILayoutStore store, Func<string> excludedListFile)
{
    /// <summary>
    /// Top-up version already applied, read from the store at load time and round-tripped
    /// into every global-options write (it is not part of the options model) so the
    /// one-time migration stays one-time.
    /// </summary>
    public int? AppliedDefaultsVersion { get; private set; }

    /// <summary>
    /// The ':'-prefixed lines found in the file, kept aside at load and re-emitted on top
    /// at every write. The daemon skips them (<c>daemon::load_excluded</c>), so they are
    /// not exclusions and have no business in the options model — the seeded
    /// <see cref="ExcludedProcessDefaults.Header"/> used to show up in the UI list as an
    /// entry the user could neither understand nor usefully delete. Holding them here
    /// rather than dropping them is what lets someone annotate their own file: the
    /// annotations disappear from the list, not from the disk.
    /// </summary>
    IReadOnlyList<string> _comments = [];

    /// <summary>
    /// Fill <paramref name="options"/> from the file, seeding or topping up the defaults as
    /// needed. <paramref name="global"/> is the options document as READ from the store: it
    /// carries the applied version in, and the top-up writes it back through it — mutating
    /// and rewriting the stored document rather than the model-derived one, so a migration
    /// running during a load cannot persist half-initialized options.
    /// </summary>
    public void Load(ILayoutOptions options, GlobalOptionsDto? global)
    {
        AppliedDefaultsVersion = global?.ExcludedDefaultsVersion;

        options.ExcludedList.Clear();

        var file = excludedListFile();
        if (!File.Exists(file))
        {
            // First run: seed the defaults and write the file the daemon reads. Same
            // content as LittleBigMouseClientService.CreateExcludedFile, header included,
            // since either of the two can be the one to create it. The version is
            // remembered so the next global-options write records the top-up as applied.
            _comments = [ExcludedProcessDefaults.Header];
            foreach (var entry in ExcludedProcessDefaults.All) options.ExcludedList.Add(entry);
            AppliedDefaultsVersion = ExcludedProcessDefaults.Version;
            Write(file, options.ExcludedList);
            return;
        }

        // Split the file the way the daemon does: an empty line is nothing, a ':' line is
        // a comment, everything else is an exclusion. Diverging here would mean the list
        // the user edits and the list that actually filters are not the same list.
        var comments = new List<string>();
        foreach (var line in File.ReadAllLines(file))
        {
            if (line.StartsWith(':')) { comments.Add(line); continue; }
            if (line.Length == 0) continue;
            options.ExcludedList.Add(line);
        }
        _comments = comments;

        MigrateDefaults(options.ExcludedList, global, file);
    }

    /// <summary>Rewrite the file the daemon reads, comments first. Best effort.</summary>
    public void Write(IEnumerable<string> entries) => Write(excludedListFile(), entries);

    void Write(string file, IEnumerable<string> entries)
    {
        try { File.WriteAllLines(file, _comments.Concat(entries)); }
        catch { /* the in-memory list is authoritative; the next full save retries */ }
    }

    /// <summary>
    /// One-time top-up of the default exclusion list. When new default entries ship (e.g.
    /// Xbox game folders, #494) they must reach users who already have an
    /// <c>Excluded.txt</c> — a fresh seed only covers new installs. Runs once per
    /// <see cref="ExcludedProcessDefaults.Version"/> (tracked in the store) and only when
    /// the list still holds every previous default, so a customized list — or a default
    /// the user deliberately removed later — is left untouched. Rewrites the file too,
    /// since the daemon reads it, not this in-memory list.
    /// </summary>
    void MigrateDefaults(ICollection<string> list, GlobalOptionsDto? global, string file)
    {
        if ((AppliedDefaultsVersion ?? 0) >= ExcludedProcessDefaults.Version) return;

        // Only top up a list that still holds all the previous defaults (i.e. the user kept them).
        // Separator-insensitive: pre-V2 Linux lists were seeded with the Windows-style entries.
        if (ExcludedProcessDefaults.LegacyV0.All(e => ExcludedProcessDefaults.ContainsEntry(list, e)))
        {
            var added = false;
            foreach (var entry in ExcludedProcessDefaults.All)
            {
                if (ExcludedProcessDefaults.ContainsEntry(list, entry)) continue;
                list.Add(entry);
                added = true;
            }

            if (added) Write(file, list);
        }

        AppliedDefaultsVersion = ExcludedProcessDefaults.Version;
        var dto = global ?? new GlobalOptionsDto();
        dto.ExcludedDefaultsVersion = AppliedDefaultsVersion;
        store.WriteGlobalOptions(dto);
    }
}
