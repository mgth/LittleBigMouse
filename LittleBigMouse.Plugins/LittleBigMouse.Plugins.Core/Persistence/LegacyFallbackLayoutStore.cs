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
/// <para>Until then, writes are dropped. A load can write (the excluded-list top-up records
/// its version), and the window may run before any agent has: creating the options file
/// then would make the agent skip the import and lose every 5.x layout, and writing the
/// registry would touch what 5.x still runs on. The agent redoes the top-up after its
/// import. Once the import is done, writes go to the JSON store.</para>
/// </summary>
public class LegacyFallbackLayoutStore(JsonLayoutStore json, Func<ILayoutStore> legacy) : ILayoutStore
{
    public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes)
        => json.HoldsOptions
            ? json.Read(layoutId, pnpCodes)
            : legacy().Read(layoutId, pnpCodes);

    public void WriteGlobalOptions(GlobalOptionsDto options)
    {
        if (Imported(nameof(WriteGlobalOptions))) json.WriteGlobalOptions(options);
    }

    public void WriteLayout(string layoutId, LayoutDto layout)
    {
        if (Imported(nameof(WriteLayout))) json.WriteLayout(layoutId, layout);
    }

    public void WriteModels(IReadOnlyDictionary<string, ModelDto> models)
    {
        if (Imported(nameof(WriteModels))) json.WriteModels(models);
    }

    bool Imported(string write)
    {
        if (json.HoldsOptions) return true;
        Console.Error.WriteLine(
            $"[LittleBigMouse] {write} skipped: the agent has not imported the 5.x registry yet");
        return false;
    }
}
