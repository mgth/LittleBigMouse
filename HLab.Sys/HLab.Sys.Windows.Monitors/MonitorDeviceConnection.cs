using System;
using System.Diagnostics;
using System.Globalization;
using System.Xml.Serialization;
using Avalonia;
using Avalonia.Controls;
using Microsoft.Win32;

namespace HLab.Sys.Windows.Monitors;

public class MonitorDeviceConnection : DisplayDevice
{
   [XmlIgnore]
   public new PhysicalAdapter Parent
   {
      get
      {
         if (base.Parent is PhysicalAdapter adapter) return adapter;
         throw new InvalidOperationException("Parent is not a PhysicalAdapter");
      }
      set => base.Parent = value;
   }

   public MonitorDevice Monitor { get; set; }

   class EdidDesign : Edid
   {
      public EdidDesign()
      {
         if (!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
         HKeyName = "HKLM://";
         ManufacturerCode = "SAM";
         ProductCode = "S24D300";
         Serial= "123456789";
         Week = 42;
         Year = 2024;   
         Version = "1.0";
         Digital = true;
         BitDepth = 8;
         VideoInterface = "Dvi";
         PhysicalWidth = 600;
         PhysicalHeight = 340;
         Model = "S24D300";
         SerialNumber = "S/N: 123456789";
         Gamma = 2.2;
         DpmsStandbySupported = true;
         DpmsSuspendSupported = true;
         DpmsActiveOffSupported = true;
         YCrCb444Support = true;
         YCrCb422Support = true;
         sRGB = 0.98;
         RedX = 0.64;
         RedY = 0.33;
         GreenX = 0.3;
         GreenY = 0.6;
         BlueX = 0.15;
         BlueY = 0.06;
         WhiteX = 0.3127;
         WhiteY = 0.3127;
         Checksum = int.MinValue;
      }

   }

   public static MonitorDeviceConnection MonitorDesign
   {
      get
      {
         if (!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");

         return new MonitorDeviceConnection
         {
            Monitor = new MonitorDevice
            {
               Edid = new EdidDesign()
            },
         };
      }
   }

   //------------------------------------------------------------------------
   public override bool Equals(object obj) => obj is MonitorDeviceConnection other ? Id == other.Id : base.Equals(obj);


   public override int GetHashCode()
   {
      return ("DisplayMonitor" + Id).GetHashCode();
   }

   void OpenRegKey(string keyString)
   {

      keyString = keyString.Replace(@"\MACHINE\", @"\HKEY_LOCAL_MACHINE\");
      keyString = keyString.Replace(@"\USER\", @"\HKEY_CURRENT_USER\");

      using (var key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit", true))
      {

         if (key == null) return;
         var value = key.GetValue("LastKey").ToString();

         var list = value.Split('\\');
         if (list.Length > 0)
         {
            keyString = keyString.Replace(@"\REGISTRY\", @$"{list[0]}\");
            key.SetValue("LastKey", keyString);
         }
      }

      Process.Start("regedit.exe");
   }


   public void DisplayValues(Action<string, string, Action?, bool> addValue)
   {
      //addValue("Registry", Edid.HKeyName, () => { OpenRegKey(Edid.HKeyName); }, false);
      //addValue("Microsoft Id", PhysicalId, null, false);

      if (Parent != null)
      {
         // EnumDisplaySettings
         addValue("", "EnumDisplaySettings", null, true);
         addValue("DisplayOrientation", Parent.CurrentMode?.DisplayOrientation.ToString() ?? "", null, false);
         addValue("Position", Parent.CurrentMode?.Position.ToString() ?? "", null, false);
         addValue("Pels", Parent.CurrentMode?.Pels.ToString() ?? "", null, false);
         addValue("BitsPerPixel", Parent.CurrentMode?.BitsPerPixel.ToString() ?? "", null, false);
         addValue("DisplayFrequency", Parent.CurrentMode?.DisplayFrequency.ToString() ?? "", null, false);
         addValue("DisplayFlags", Parent.CurrentMode?.DisplayFlags.ToString() ?? "", null, false);
         addValue("DisplayFixedOutput", Parent.CurrentMode?.DisplayFixedOutput.ToString() ?? "", null, false);

         // GetDeviceCaps
         addValue("", "GetDeviceCaps", null, true);
         addValue("Size", Parent.Capabilities.Size.ToString(), null, false);
         addValue("Res", Parent.Capabilities.Resolution.ToString(), null, false);
         addValue("LogPixels", Parent.Capabilities.LogPixels.ToString(), null, false);
         addValue("BitsPixel", Parent.Capabilities.BitsPixel.ToString(), null, false);
         //AddValue("Color Planes", Monitor.Adapter.DeviceCaps.Planes.ToString());
         addValue("Aspect", Parent.Capabilities.Aspect.ToString(), null, false);
         //AddValue("BltAlignment", Monitor.Adapter.DeviceCaps.BltAlignment.ToString());

         //GetDpiForMonitor
         addValue("", "GetDpiForMonitor", null, true);
         addValue("EffectiveDpi", Parent.EffectiveDpi.ToString(), null, false);
         addValue("AngularDpi", Parent.AngularDpi.ToString(), null, false);
         addValue("RawDpi", Parent.RawDpi.ToString(), null, false);

         // GetMonitorInfo
         addValue("", "GetMonitorInfo", null, true);
         addValue("Primary", Parent.Primary.ToString(), null, false);
         addValue("MonitorArea", Parent.MonitorArea.ToString(), null, false);
         addValue("WorkArea", Parent.WorkArea.ToString(), null, false);


         //// EDID
         addValue("", "EDID", null, true);
         addValue("ManufacturerCode", Monitor?.Edid?.ManufacturerCode, null, false);
         addValue("ProductCode", Monitor?.Edid?.ProductCode, null, false);
         addValue("Serial", Monitor?.Edid?.Serial, null, false);
         addValue("Model", Monitor?.Edid?.Model, null, false);
         addValue("SerialNo", Monitor?.Edid?.SerialNumber, null, false);
         addValue("SizeInMm H", Monitor?.Edid?.PhysicalWidth.ToString(), null, false);
         addValue("SizeInMm V", Monitor?.Edid?.PhysicalHeight.ToString(), null, false);
         addValue("VideoInterface", Monitor?.Edid?.VideoInterface.ToString(), null, false);

         // GetScaleFactorForMonitor
         addValue("", "GetScaleFactorForMonitor", null, true);
         addValue("ScaleFactor", Parent.ScaleFactor.ToString(CultureInfo.CurrentCulture) ?? "", null, false);

         // EnumDisplayDevices
         addValue("", "EnumDisplayDevices", null, true);
         addValue("DeviceId", Parent.Id, null, false);
         addValue("DeviceKey", Parent.DeviceKey, null, false);
         addValue("DeviceString", Parent.DeviceString, null, false);
         addValue("DeviceName", Parent.DeviceName, null, false);
         addValue("StateFlags", Parent.State.ToString(), null, false);
      }

      addValue("", "EnumDisplayDevices", null, true);
      addValue("DeviceId", Id, null, false);
      addValue("DeviceKey", DeviceKey, null, false);
      addValue("DeviceString", DeviceString, null, false);
      addValue("DeviceName", DeviceName, null, false);
      addValue("StateFlags", State.ToString(), null, false);

   }
   public override string ToString() => DeviceString;

}