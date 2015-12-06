using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography.X509Certificates;
using System.Text;
using System.Threading.Tasks;

namespace LittleBigMouse_Control
{
    public interface IPluginButton
    {
        string Caption { get; }
        bool IsActivated { get; set; }
    }
}
