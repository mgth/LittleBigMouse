#nullable enable
using System;
using System.Collections.Generic;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// The store the Windows window reads (v6, decision D2): the agent's JSON store, which is
/// the only one the agent reads and writes. Before the agent's first launch has imported
/// the 5.x registry into it — no options file yet, the agent's own marker — reads come
/// from the legacy store instead: that is exactly what the import is about to copy, so
/// the window shows the user's layout and not a first run.
/// <para>Writes always go to the JSON store. The window does not write in v6 (the agent
/// is the only writer); were it to, writing the registry would be lost to the agent, and
/// writing the JSON store first would make the agent skip the import.</para>
/// </summary>
public class LegacyFallbackLayoutStore(JsonLayoutStore json, Func<ILayoutStore> legacy) : ILayoutStore
{
    public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes)
        => json.HoldsOptions
            ? json.Read(layoutId, pnpCodes)
            : legacy().Read(layoutId, pnpCodes);

    public void WriteGlobalOptions(GlobalOptionsDto options) => json.WriteGlobalOptions(options);

    public void WriteLayout(string layoutId, LayoutDto layout) => json.WriteLayout(layoutId, layout);

    public void WriteModels(IReadOnlyDictionary<string, ModelDto> models) => json.WriteModels(models);
}
