using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace LittleBigMouse.Ui.Avalonia.Main;

public class TrayIconViewModel
{
    public string Icon { get; set; } = "avares://LittleBigMouse.Ui.Avalonia/Assets/MainIcon.ico";

    public bool Enabled { get; set; } = true;


}