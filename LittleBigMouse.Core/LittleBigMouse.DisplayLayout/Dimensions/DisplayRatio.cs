/*
  LittleBigMouse.DisplayLayout
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.DisplayLayout.

    LittleBigMouse.DisplayLayout is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.DisplayLayout is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Runtime.Serialization;
using HLab.Base.ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

[DataContract]
public abstract class DisplayRatio : SavableReactiveModel, IDisplayRatio, IEquatable<IDisplayRatio>
{
    [DataMember]
    public double X => XValue;
    protected abstract double XValue { get; }
    [DataMember]
    public double Y => YValue;
    protected abstract double YValue { get; }

    public bool IsUnary => Math.Abs(X - 1) < double.Epsilon && Math.Abs(Y - 1) < double.Epsilon;

    public bool Equals(IDisplayRatio other)
    {
        if (other == null) return false;
        return !(Math.Abs(X - other.X) > double.Epsilon || Math.Abs(Y - other.Y) > double.Epsilon);
    }

    public override string ToString() => $"({X},{Y})";

}

/// <summary>
/// Base class for ratios whose components are explicitly editable.
/// </summary>
public abstract class MutableDisplayRatio : DisplayRatio, IMutableDisplayRatio
{
    public new abstract double X { get; set; }
    public new abstract double Y { get; set; }

    protected sealed override double XValue => X;
    protected sealed override double YValue => Y;
}
