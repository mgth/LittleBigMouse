using System;
using System.Collections.Generic;
using System.Linq;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Canonical default exclusion list: game-launcher install-path fragments whose windows LBM should
/// leave alone (matched as substrings against the foreground process path by the daemon). Single
/// source of truth shared by the seed file (<c>CreateExcludedFile</c>), the one-time top-up
/// migration (<c>LayoutPersistence</c>) and the "Add defaults" button (<c>LbmOptionsViewModel</c>).
/// Per-OS: the fragments describe install conventions, which differ between Windows and Linux.
/// </summary>
public static class ExcludedProcessDefaults
{
    /// <summary>
    /// Bumped whenever entries are added to <see cref="All"/>, to drive the one-time migration that
    /// tops up the lists of users who kept the stock defaults. A later manual removal is respected
    /// (the migration only runs once per version). History: 1 = <c>\XboxGames\</c> (#494);
    /// 2 = per-OS lists, Linux entries added (#515).
    /// </summary>
    public const int Version = 2;

    /// <summary>Header comment written atop a freshly seeded file (':'-prefixed lines are ignored by the daemon).</summary>
    public const string Header = ":Excluded processes";

    /// <summary>The defaults that shipped before <see cref="Version"/> 1 (on both OSes).</summary>
    public static readonly IReadOnlyList<string> LegacyV0 =
    [
        @"\Epic Games\",
        @"\steamapps\",
        @"\Riot Games\",
    ];

    /// <summary>
    /// Windows defaults. <c>\XboxGames\</c> was added in Version 1 (#494): Xbox / Game Pass
    /// titles install under <c>…\XboxGames\…\Content\</c> regardless of the chosen install drive.
    /// </summary>
    public static readonly IReadOnlyList<string> Windows =
    [
        .. LegacyV0,
        @"\XboxGames\",
    ];

    /// <summary>
    /// Linux defaults, in the native separator style. The daemon matches separator-insensitively
    /// (<c>\</c> == <c>/</c>), so these also cover the Windows-style command lines Wine/Proton
    /// games expose (<c>Z:\…\steamapps\common\…\Game.exe</c>, <c>C:\Riot Games\…</c>).
    /// No <c>/XboxGames/</c>: Game Pass does not exist on Linux.
    /// </summary>
    public static readonly IReadOnlyList<string> Linux =
    [
        "/steamapps/",   // Steam, native and Proton (…/Steam/steamapps/common/…)
        "/Epic Games/",  // Epic launcher inside a Wine prefix (C:\Program Files\Epic Games\…)
        "/Riot Games/",  // Riot inside a Wine prefix (C:\Riot Games\…)
        "/Heroic/",      // Heroic Games Launcher library (default ~/Games/Heroic)
        "/Games/",       // ~/Games — Lutris default install dir, common convention
    ];

    /// <summary>The current default entries for the OS we are running on.</summary>
    public static readonly IReadOnlyList<string> All =
        OperatingSystem.IsWindows() ? Windows : Linux;

    /// <summary>
    /// Separator-insensitive containment (<c>\</c> == <c>/</c>, like the daemon's matching): a list
    /// seeded with the pre-<see cref="Version"/>-2 Windows-style entries already covers the
    /// slash-style Linux defaults — the top-up must not duplicate them.
    /// </summary>
    public static bool ContainsEntry(IEnumerable<string> list, string entry)
        => list.Any(e => Normalize(e) == Normalize(entry));

    static string Normalize(string s) => s.Replace('\\', '/');
}
