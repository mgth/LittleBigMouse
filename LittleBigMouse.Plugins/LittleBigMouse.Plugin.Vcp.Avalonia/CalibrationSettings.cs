#nullable enable
using System.IO;
using System.Xml.Serialization;

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Calibration parameters, persisted twice: a common file shared by every
/// monitor, and one file per monitor holding the always-per-monitor flags
/// (UseCustom, TestPairs) plus the custom values used when UseCustom is on
/// (kept as a seed even while it is off).
/// </summary>
public class CalibrationSettings
{
   [XmlAttribute] public bool UseCustom;
   [XmlAttribute] public bool TestPairs = true;
   [XmlAttribute] public double ColorTemp = 6500;
   [XmlAttribute] public string Observer = "CIE_1931_2";
   [XmlAttribute] public string Speed = "Normal";
}

public static class CalibrationSettingsStore
{
   // Same roots as ProbeLut's Luminance.xml: Windows keeps the historical
   // Mgth store, Linux follows the port's convention.
   static string Root => OperatingSystem.IsWindows()
       ? Path.Combine(
           Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
           "Mgth", "LittleBigMouse")
       : Path.Combine(
           Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
           "LittleBigMouse");

   static string GlobalPath => Path.Combine(Root, "calibration.xml");
   static string MonitorPath(string monitorId) => Path.Combine(Root, monitorId, "calibration.xml");

   public static CalibrationSettings? LoadGlobal() => Load(GlobalPath);
   public static CalibrationSettings? LoadMonitor(string monitorId) => Load(MonitorPath(monitorId));

   public static void SaveGlobal(CalibrationSettings settings) => Save(GlobalPath, settings);
   public static void SaveMonitor(string monitorId, CalibrationSettings settings) => Save(MonitorPath(monitorId), settings);

   static CalibrationSettings? Load(string path)
   {
      try
      {
         if (!File.Exists(path)) return null;
         using var reader = new StreamReader(path);
         return new XmlSerializer(typeof(CalibrationSettings)).Deserialize(reader) as CalibrationSettings;
      }
      catch (Exception)
      {
         return null;
      }
   }

   static void Save(string path, CalibrationSettings settings)
   {
      try
      {
         Directory.CreateDirectory(Path.GetDirectoryName(path)!);
         using var writer = new StreamWriter(path);
         new XmlSerializer(typeof(CalibrationSettings)).Serialize(writer, settings);
      }
      catch (Exception)
      {
      }
   }
}
