using System;
using System.Runtime.Serialization;
using HLab.Base.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Defines monitor physical properties (size, brand, model, etc...)
/// </summary>
[DataContract]
public class PhysicalMonitorModel : SavableReactiveModel
{
    public static PhysicalMonitorModel Design
    {
        get
        {
            //if(!Avalonia.Controls.Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
            return new PhysicalMonitorModel("PNP0000")
            {
            };
        }
    }

    // TODO : some monitors may have different pnpcode for each source.
    public string PnpCode { get; }

    public PhysicalMonitorModel(string pnpCode)
    {
        PnpCode = pnpCode;
        PhysicalSize = new DisplaySizeInMm(/*this*/);

        this.WhenAnyValue(e => e.PhysicalSize.Saved)
            .Subscribe(e =>
            {
                if (e) return;
                Saved = false;
            });
    }

    [DataMember]
    public string PnpDeviceName { get; set => SetUnsavedValue(ref field, value); }

    /// <summary>
    /// Icon path for brand logo
    /// </summary>
    public string Logo { get; set => this.RaiseAndSetIfChanged(ref field, value); }

    [DataMember] public DisplaySizeInMm PhysicalSize { get; }

    /// <summary>
    /// The size the platform detected for this model, captured by the first load that
    /// applies a store over it; null until then. The store migrations compare a stored
    /// size with THIS, never with the current size: a reload of the same model (Undo, an
    /// agent applying a document over the profile it loaded) would otherwise take the
    /// profile's own size for a detection (#794 review). Not persisted.
    /// </summary>
    public (double Width, double Height)? DetectedSize { get; set; }

    /// <summary>
    /// The stored size as read, while its pre-5.4.1 orientation migration could not be
    /// decided because the detected size had no shape yet (see
    /// <c>LayoutMigrations.StoredSizeUnresolved</c>); null otherwise. Neither persisted nor
    /// tracked for <c>Saved</c>: it only tells the next save not to mark this size intrinsic
    /// unless the user has changed it since.
    /// </summary>
    public (double Width, double Height)? UnresolvedStoredSize { get; set; }


}