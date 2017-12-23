using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using HLab.Notify;

namespace HLab.Windows.Monitors
{
    class PhysicalAdapter : NotifierObject
    {
        public string DeviceString
        {
            get => this.Get<string>();
            internal set => this.Set(value ?? "");
        }
    }
}
