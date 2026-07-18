#nullable enable
using System.Collections.Generic;

namespace LittleBigMouse.Plugins.Persistence;

/// <summary>
/// Dumb storage backend behind <see cref="LayoutPersistence"/>: it moves DTOs in and out of
/// the OS store (registry on Windows, JSON files on Linux) and knows nothing about the
/// layout model. All mapping and semantics live in the engine, once.
/// </summary>
public interface ILayoutStore
{
    /// <summary>
    /// One read of everything relevant to a layout: the global options, the layout document
    /// and the stored models for the given PnP codes. Absent data comes back as null (or a
    /// missing dictionary entry). Reads are PURE: they must never create keys nor seed
    /// values in the store.
    /// </summary>
    LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes);

    /// <summary>Write the app-level options. Null fields are skipped, never deleted.</summary>
    void WriteGlobalOptions(GlobalOptionsDto options);

    /// <summary>Write one layout document (options + monitors). Null fields are skipped.</summary>
    void WriteLayout(string layoutId, LayoutDto layout);

    /// <summary>Upsert the given monitor models; stored models not listed are left untouched.</summary>
    void WriteModels(IReadOnlyDictionary<string, ModelDto> models);
}

public class LayoutStoreData
{
    public GlobalOptionsDto? GlobalOptions { get; set; }
    public LayoutDto? Layout { get; set; }
    public Dictionary<string, ModelDto> Models { get; set; } = [];
}
