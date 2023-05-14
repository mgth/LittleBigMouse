using System.Collections.Generic;
using System.Globalization;
using System.Runtime.CompilerServices;
using System.Runtime.Serialization;

using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;

using Newtonsoft.Json;
using ReactiveUI;
using static HLab.Sys.Windows.API.WinDef;

namespace LittleBigMouse.DisplayLayout.Monitors
{
    [DataContract]
    public class PhysicalMonitorModel : ReactiveObject
    {
        public static PhysicalMonitorModel Design => new PhysicalMonitorModel("PNP0000")
        {

        };

        // TODO : some monitors may have different pnpcode for each source.
         public string PnpCode { get; }

        public PhysicalMonitorModel(string pnpCode)
        {
            PnpCode = pnpCode;
            PhysicalSize = new DisplaySizeInMm(/*this*/);
        }

        bool SetValue<TRet>(ref TRet backingField, TRet value, [CallerMemberName] string propertyName = null)
        {
            using (DelayChangeNotifications())
            {
                if (!EqualityComparer<TRet>.Default.Equals(backingField, value))
                {
                    this.RaisePropertyChanging(propertyName);
                    backingField = value;
                    Saved = false;
                    this.RaisePropertyChanged(propertyName);
                    return true;
                }

                return false;
            }
        }

        [DataMember]
        public string PnpDeviceName
        {
            get => _pnpDeviceName;
            set => SetValue(ref _pnpDeviceName, value);
        }
        string _pnpDeviceName;

        /// <summary>
        /// Icon path for brand logo
        /// </summary>
        public string Logo
        {
            get => _logo;
            set => this.RaiseAndSetIfChanged(ref _logo, value);
        }
        string _logo;


        [DataMember] public DisplaySizeInMm PhysicalSize { get; }

        public bool Saved
        {
            get => _saved;
            set => this.RaiseAndSetIfChanged(ref _saved, value);
        }
        bool _saved;

    }
}
