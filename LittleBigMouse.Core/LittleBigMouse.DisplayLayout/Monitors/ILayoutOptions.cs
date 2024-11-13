#nullable enable
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Runtime.CompilerServices;

namespace LittleBigMouse.DisplayLayout.Monitors;

public interface IExcludedListItem
{
    public class Design(string name) : IExcludedListItem
    {
        public string Name { get; } = name;
    }
    string Name { get; }
}

public interface ILayoutOptions : INotifyPropertyChanged // Change IPropertyChange to INotifyPropertyChanged
{
   public class Design : ILayoutOptions
   {
      public string GetConfigPath(string fileName, bool create) => "";

      public double MinimalMaxTravelDistance { get; set; } = 20;
      public bool AllowDiscontinuity { get; set; } = false;
      public bool AllowOverlaps { get; set; } = false;
      public bool LoopX { get; set; } = false;
      public bool LoopY { get; set; } = false;
      public double MaxTravelDistance { get; set; } = 200;
      public bool AdjustPointer { get; set; } = false;
      public bool AdjustSpeed { get; set; } = false;
      public string Algorithm { get; set; } = "Strait";
      public string Priority { get; set; } = "Normal";
      public string PriorityUnhooked { get; set; } = "Below";
      public bool IsUnaryRatio { get; set; } = false;
      public bool Enabled { get; set; } = true;
      public bool AutoUpdate { get; set; } = true;
      public bool LoadAtStartup { get; set; } = false;
      public bool HomeCinema { get; set; } = false;
      public bool Pinned { get; set; } = false;
      public bool StartMinimized { get; set; } = false;
      public bool StartElevated { get; set; } = false;
      public bool Elevated { get; set; } = true;
      public int DaemonPort { get; set; } = 25196;

      public bool AdjustSpeedAllowed { get; } = false;
      public ObservableCollection<string> ExcludedList { get; } = new ObservableCollection<string> { "/game/", "/another/game/" };
      public bool Saved { get; set; } = false;
      public bool LoopAllowed { get; } = true;
      public event PropertyChangedEventHandler? PropertyChanged;

      protected virtual void OnPropertyChanged([CallerMemberName] string? propertyName = null)
      {
         PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(propertyName));
      }

      protected bool SetField<T>(ref T field, T value, [CallerMemberName] string? propertyName = null)
      {
         if (EqualityComparer<T>.Default.Equals(field, value)) return false;
         field = value;
         OnPropertyChanged(propertyName);
         return true;
      }
   }

   public string GetConfigPath(string fileName, bool create);

   double MinimalMaxTravelDistance { get; set; }

   /// <summary>
   /// Allow for discontinuities between monitors.
   /// </summary>
   bool AllowDiscontinuity { get; set; }

   /// <summary>
   /// Allow overlaps between monitors.
   /// </summary>
   bool AllowOverlaps { get; set; }

   /// <summary>
   /// Allow mouse cursor looping in X direction
   /// </summary>
   bool LoopX { get; set; }

   /// <summary>
   /// Allow mouse cursor looping in Y direction
   /// </summary>
   bool LoopY { get; set; }


   /// <summary>
   /// Max distance allowed to reach monitor
   /// Helps to prevent cursor to reach monitor too far
   /// </summary>
   double MaxTravelDistance { get; set; }

   /// <summary>
   /// Adjust pointer size with display pixel to dip ratio
   /// </summary>
   bool AdjustPointer { get; set; }

   /// <summary>
   /// Adjust speed with display pixel to dip ratio
   /// </summary>
   bool AdjustSpeed { get; set; }

   /// <summary>
   /// algorithm to be used for mouse movements
   /// - Strait
   /// - CornerCrossing
   /// </summary>
   string Algorithm { get; set; }


   /// <summary>
   /// Daemon process priority
   /// </summary>
   string Priority { get; set; }

   /// <summary>
   /// Daemon process priority when not hooked
   /// </summary>
   string PriorityUnhooked { get; set; }

   /// <summary>
   /// True if all sources have a pixel to dip ratio of 1
   /// </summary>
   bool IsUnaryRatio { get; set; }

   /// <summary>
   /// Mouse management enabled
   /// </summary>
   bool Enabled { get; set; }

   /// <summary>
   /// Allow software to check for updates online
   /// </summary>
   bool AutoUpdate { get; set; }

   /// <summary>
   /// Allow software sto start with user logon
   /// </summary>
   bool LoadAtStartup { get; set; }

   /// <summary>
   /// Experimental : Sleep monitors not containing mouse cursor after a delay 
   /// </summary>
   bool HomeCinema { get; set; }

   /// <summary>
   /// Keep ui window on top
   /// </summary>
   bool Pinned { get; set; }

   /// <summary>
   /// Start minimized in tray
   /// </summary>
   bool StartMinimized { get; set; }

   /// <summary>
   /// Start with elevated privileges
   /// </summary>
   bool StartElevated { get; set; }

   /// <summary>
   /// Current elevated state
   /// </summary>
   bool Elevated { get; set; }

   /// <summary>
   /// Daemon port for TCP communication
   /// </summary>
   public int DaemonPort { get; set; }

   public bool LoopAllowed { get; }
   public ObservableCollection<string> ExcludedList { get; }
   bool Saved { get; set; }
}
