/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Avalonia;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

public class BorderResistanceViewModel : ViewModel<PhysicalMonitor>
{
    public BorderSideViewModel? Left
    {
        get;
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public BorderSideViewModel? Top
    {
        get;
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public BorderSideViewModel? Right
    {
        get;
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public BorderSideViewModel? Bottom
    {
        get;
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public BorderSectionViewModel? Selected
    {
        get;
        set => this.RaiseAndSetIfChanged(ref field, value);
    }

    protected override PhysicalMonitor? OnModelChanging(PhysicalMonitor? oldModel, PhysicalMonitor? newModel)
    {
        if (newModel == null)
        {
            Left = Top = Right = Bottom = null;
        }
        else
        {
            Left = new BorderSideViewModel(newModel, BorderSideKind.Left, newModel.BorderResistance.Left);
            Top = new BorderSideViewModel(newModel, BorderSideKind.Top, newModel.BorderResistance.Top);
            Right = new BorderSideViewModel(newModel, BorderSideKind.Right, newModel.BorderResistance.Right);
            Bottom = new BorderSideViewModel(newModel, BorderSideKind.Bottom, newModel.BorderResistance.Bottom);
        }

        return base.OnModelChanging(oldModel, newModel);
    }

    // The pixel scale of each edge is published by its own strip, whose length is
    // the edge length by construction; deriving it here from the overlay bounds was
    // wrong as soon as the strips stopped spanning the full rectangle.

    public void Select(BorderSectionViewModel? section)
    {
        if (Selected != null) Selected.IsSelected = false;
        Selected = section;
        if (section != null) section.IsSelected = true;
    }
}