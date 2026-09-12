#nullable enable
using System.Collections.Generic;
using System.Linq;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// A layout as a frontend hands it to the agent (v6, phase 4): the agent is the only
/// writer, so the frontends send **what they would have saved** and the agent writes it.
/// <para>
/// It is exactly what <see cref="LayoutPersistence.Save"/> writes — the app-level options,
/// the layout document, its models — plus the excluded list, which lives in its own file.
/// The Rust side reads it as <c>lbm_store::LayoutDocument</c> and applies it to its own
/// copy of the layout, which then holds what a save followed by a load would have left.
/// </para>
/// <para>
/// The members are the store's own, so nothing new has to agree between the two sides:
/// the same DTOs, the same names, the same absent-for-null rule.
/// </para>
/// </summary>
public static class AgentDocument
{
    /// <summary>What a save of <paramref name="layout"/> would write.</summary>
    public static AgentDocumentDto Of(MonitorsLayout layout) => new()
    {
        // The defaults version belongs to the store, not to what the frontend edits.
        GlobalOptions = LayoutDtoMapper.ToGlobalOptionsDto(layout.Options, null),
        Layout = LayoutDtoMapper.ToLayoutDto(layout),
        Models = layout.PhysicalMonitors
            .Select(m => m.Model)
            .DistinctBy(m => m.PnpCode)
            .ToDictionary(m => m.PnpCode, LayoutDtoMapper.ToDto),
        Excluded = layout.Options.ExcludedList.ToList(),
    };
}

/// <summary>The document's shape on the wire (`lbm_store::LayoutDocument`).</summary>
public sealed class AgentDocumentDto
{
    public GlobalOptionsDto? GlobalOptions { get; set; }
    public LayoutDto? Layout { get; set; }
    public Dictionary<string, ModelDto> Models { get; set; } = [];

    /// <summary>The excluded processes; absent leaves them as they are.</summary>
    public List<string>? Excluded { get; set; }
}
