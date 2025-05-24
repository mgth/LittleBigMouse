using HLab.Base;

namespace LittleBigMouse.DisplayLayout.Dimensions;

public interface IDisplayRatio : ISavable
{
   double X { get; set; }
   double Y { get; set; }
   bool IsUnary { get; }
}