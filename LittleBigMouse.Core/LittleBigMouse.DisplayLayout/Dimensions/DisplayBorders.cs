using System;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayBorders : ReactiveObject
{
   static double NonNegative(double value) => Math.Max(0.0, value);

   public double Left { get; set => this.RaiseAndSetIfChanged(ref field, NonNegative(value)); }

   public double Top { get; set => this.RaiseAndSetIfChanged(ref field, NonNegative(value)); }

   public double Right { get; set => this.RaiseAndSetIfChanged(ref field, NonNegative(value)); }

   public double Bottom { get; set => this.RaiseAndSetIfChanged(ref field, NonNegative(value)); }
}
