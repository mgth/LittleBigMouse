using System.Collections.Generic;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Canonical default exclusion list: game-launcher install-path fragments whose windows LBM should
/// leave alone (matched as substrings against the foreground process path by the daemon). Single
/// source of truth shared by the seed file (<c>CreateExcludedFile</c>), the one-time top-up
/// migration (<c>LayoutPersistence</c>) and the "Add defaults" button (<c>LbmOptionsViewModel</c>).
/// </summary>
public static class ExcludedProcessDefaults
{
    /// <summary>
    /// Bumped whenever entries are added to <see cref="All"/>, to drive the one-time migration that
    /// tops up the lists of users who kept the stock defaults. A later manual removal is respected
    /// (the migration only runs once per version).
    /// </summary>
    public const int Version = 1;

    /// <summary>Header comment written atop a freshly seeded file (':'-prefixed lines are ignored by the daemon).</summary>
    public const string Header = ":Excluded processes";

    /// <summary>The defaults that shipped before <see cref="Version"/> 1.</summary>
    public static readonly IReadOnlyList<string> LegacyV0 =
    [
        @"\Epic Games\",
        @"\steamapps\",
        @"\Riot Games\",
    ];

    /// <summary>
    /// The current default entries. <c>\XboxGames\</c> was added in Version 1 (#494): Xbox / Game Pass
    /// titles install under <c>…\XboxGames\…\Content\</c> regardless of the chosen install drive.
    /// </summary>
    public static readonly IReadOnlyList<string> All =
    [
        .. LegacyV0,
        @"\XboxGames\",
    ];
}
