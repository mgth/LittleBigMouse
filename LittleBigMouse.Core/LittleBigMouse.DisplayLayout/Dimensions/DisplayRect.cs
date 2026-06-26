using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class DisplayRect : ReactiveObject
{
   public double X { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Y  { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Width { get; set => this.RaiseAndSetIfChanged(ref field, value); }

   public double Height { get; set => this.RaiseAndSetIfChanged(ref field, value); }
}