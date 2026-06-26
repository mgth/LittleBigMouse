using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayBorders : ReactiveObject
{
   public double Left { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Top { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Right { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Bottom { get; set => this.RaiseAndSetIfChanged(ref field, value); }
}