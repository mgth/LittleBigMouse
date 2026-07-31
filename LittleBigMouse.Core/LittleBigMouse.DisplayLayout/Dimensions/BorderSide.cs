using System;
using System.Collections.Generic;
using System.Linq;
using System.Reactive.Linq;
using System.Text.Json.Serialization;
using DynamicData;
using HLab.Base.ReactiveUI;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// One edge of a monitor: a default resistance pair, plus the sections overriding
/// it over part of the edge.
/// </summary>
public class BorderSide : SavableReactiveModel, IBorderSide
{
    public BorderSide()
    {
        Sections.DisposeWith(this);

        // Same shape as PhysicalMonitor.Sources: AutoRefresh so editing a section
        // in place — not just adding or removing one — marks the side, and through
        // it the monitor, as needing a save.
        Sections.Connect()
            .AutoRefresh(e => e.Saved)
            .ToCollection()
            .Do(OnSectionsChanged)
            .Subscribe().DisposeWith(this);

        // Removing a section leaves every remaining one saved, so the check above
        // would miss it. Skip(1): CountChanged replays the current count on
        // subscribe, and construction is not an edit.
        Sections.CountChanged
            .Skip(1)
            .Subscribe(_ => Saved = false).DisposeWith(this);

    }

    /// <summary>
    /// The editable collection. Hidden from the JSON diagnostics export: a
    /// SourceList has no meaningful serialized shape (Count, CountChanged…), and
    /// the export round-trips through <see cref="SectionsSnapshot"/> instead.
    /// </summary>
    [JsonIgnore]
    public ISourceList<BorderSection> Sections { get; } = new SourceList<BorderSection>();

    /// <summary>
    /// Materialized on change rather than projected on each read: the link compiler
    /// walks this list for every candidate interval of every edge. Typed as the
    /// interface so the diagnostics export writes the six meaningful fields and
    /// nothing else.
    /// </summary>
    [JsonPropertyName("Sections")]
    public IReadOnlyList<IBorderSection> SectionsSnapshot { get; private set; } = [];

    IReadOnlyList<IBorderSection> IBorderSide.Sections => SectionsSnapshot;

    void OnSectionsChanged(IReadOnlyCollection<BorderSection> sections)
    {
        SectionsSnapshot = [.. sections];
        this.RaisePropertyChanged(nameof(SectionsSnapshot));

        if (sections.Any(s => !s.Saved)) Saved = false;
    }
}
