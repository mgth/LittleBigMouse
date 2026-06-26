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


}