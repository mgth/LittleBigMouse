using System.Runtime.Serialization;

namespace HLab.Sys.Windows.Monitors;

public class DeviceState
{
    [DataMember] public bool AttachedToDesktop { get; set; }
    [DataMember] public bool MultiDriver { get; set; }
    [DataMember] public bool PrimaryDevice { get; set; }
    [DataMember] public bool MirroringDriver { get; set; }
    [DataMember] public bool VgaCompatible { get; set; }
    [DataMember] public bool Removable { get; set; }
    [DataMember] public bool ModesPruned { get; set; }
    [DataMember] public bool Remote { get; set; }
    [DataMember] public bool Disconnect { get; set; }
}