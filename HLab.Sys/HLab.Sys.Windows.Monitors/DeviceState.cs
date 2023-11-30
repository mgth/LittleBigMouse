using System.Runtime.Serialization;
using System.Text;

namespace HLab.Sys.Windows.Monitors;

public class DeviceState
{
    public override string ToString()
    {
        var sb = new StringBuilder();
        if(AttachedToDesktop) sb.Append("AttachedToDesktop|");
        if(MultiDriver) sb.Append("MultiDriver|");
        if(PrimaryDevice) sb.Append("PrimaryDevice|");
        if(MirroringDriver) sb.Append("MirroringDrive|r");
        if(VgaCompatible) sb.Append("VgaCompatible|");
        if(Removable) sb.Append("Removable|");
        if(ModesPruned) sb.Append("ModesPruned|");
        if(Remote) sb.Append("Remote|");
        if(Disconnect) sb.Append("Disconnect|");
        if (sb.Length>0) sb.Length --;
        return sb.ToString();
    }
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