using HLab.Base;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// Read-only ratio view. Ratios produced by calculations expose this contract.
/// </summary>
public interface IDisplayRatio : ISavable
{
   double X { get; }
   double Y { get; }
   bool IsUnary { get; }
}

/// <summary>
/// Ratio whose two components are independently editable.
/// </summary>
public interface IMutableDisplayRatio : IDisplayRatio
{
   new double X { get; set; }
   new double Y { get; set; }
}
