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

using System;
using System.Collections.Generic;
using System.Reactive.Linq;
using Avalonia;
using Avalonia.Platform;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins.Avalonia;
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
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public BorderResistanceViewModel()
    {
        BorderSectionSelection.Changed += OnSelectionChanged;

        // Toggled from any monitor, reflected on all of them.
        ScreenSectionsOverlay.Changed += OnOverlayChanged;
    }

    void OnOverlayChanged() => this.RaisePropertyChanged(nameof(ShowOnScreens));

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

    /// <summary>
    /// Mirror the sections onto the real monitors, like the rulers do. The map is
    /// drawn at whatever zoom the window sits at, so it cannot answer the question
    /// the feature exists for — does this passage actually line up with the taskbar
    /// button, this wall with the window controls.
    /// <para>
    /// A proxy over the shared overlay: every monitor of the map shows this toggle,
    /// and they are all the same switch.
    /// </para>
    /// </summary>
    public bool ShowOnScreens
    {
        get => ScreenSectionsOverlay.Visible;
        set => ScreenSectionsOverlay.Toggle(Model?.Layout, value);
    }

    /// <summary>
    /// Follow the shared selection so the editor opens on whatever was clicked —
    /// in the map or on a real screen — and closes when the section is deleted.
    /// </summary>
    void OnSelectionChanged(BorderSection? selected)
    {
        if (selected == null)
        {
            Selected = null;
            return;
        }

        foreach (var side in (BorderSideViewModel?[])[Left, Top, Right, Bottom])
        {
            if (side == null) continue;

            foreach (var section in side.Sections)
            {
                if (!ReferenceEquals(section.Model, selected)) continue;
                Selected = section;
                return;
            }
        }

        // The selected section belongs to another monitor: this editor closes.
        Selected = null;
    }

    public override void OnDispose()
    {
        BorderSectionSelection.Changed -= OnSelectionChanged;
        ScreenSectionsOverlay.Changed -= OnOverlayChanged;

        // Belt and braces alongside the view's OnUnloaded: the bands are top-level
        // windows, so nothing else would ever close them.
        ScreenSectionsOverlay.Toggle(null, false);

        base.OnDispose();
    }
}