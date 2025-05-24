using HLab.Base.ReactiveUI;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public class BorderResistance : SavableReactiveModel, IBorderResistance
{
   public double Left { get; set => SetUnsavedValue(ref field, value); }

   public double Top { get; set => SetUnsavedValue(ref field, value); }

   public double Right { get; set => SetUnsavedValue(ref field, value); }

   public double Bottom { get; set => SetUnsavedValue(ref field, value); }
}