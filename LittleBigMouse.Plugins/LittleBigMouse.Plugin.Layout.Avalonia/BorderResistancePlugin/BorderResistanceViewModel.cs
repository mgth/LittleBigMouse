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

    /// <summary>
    /// Mirror the sections onto the real monitors, like the rulers do. The map is
    /// drawn at whatever zoom the window sits at, so it cannot answer the question
    /// the feature exists for — does this passage actually line up with the taskbar
    /// button, this wall with the window controls.
    /// </summary>
    public bool ShowOnScreens
    {
        get;
        set
        {
            this.RaiseAndSetIfChanged(ref field, value);
            UpdateScreenOverlay();
        }
    }

    /// <summary>
    /// Shared, not per-monitor: the overlay covers every screen, so the view model
    /// of a second monitor toggling it must replace the bands rather than draw a
    /// second set over the first.
    /// </summary>
    static readonly List<ScreenSectionsWindow> Overlay = [];

    /// <summary>Band thickness in DIPs. Thinner than the 100-DIP rulers: it carries no digits.</summary>
    const double BandThickness = 24.0;

    // No refresh entry point on purpose: the on-screen bands are the same control
    // reading the same section collection, so they follow every edit on their own.
    // Rebuilding them after a gesture would close and reopen four windows per
    // screen, which flickers and would cut an interaction happening on the screen.

    public void UpdateScreenOverlay()
    {
        foreach (var window in Overlay) window.Close();
        Overlay.Clear();

        if (!ShowOnScreens || Model?.Layout == null) return;

        foreach (var source in Model.Layout.PhysicalSources)
        {
            if (!source.Source.AttachedToDesktop) continue;

            // Every edge, like the rulers: an edge with nothing on it still tells you
            // that there is nothing on it, and a band that appears and disappears
            // depending on its contents is harder to read than a constant frame.
            var monitor = source.Monitor;
            var windows = new List<(BorderSideKind Kind, ScreenSectionsWindow Window)>();

            foreach (var kind in (BorderSideKind[])
                     [BorderSideKind.Top, BorderSideKind.Bottom, BorderSideKind.Left, BorderSideKind.Right])
            {
                var side = new BorderSideViewModel(monitor, kind, BorderSideViewModel.SideOf(monitor, kind));
                windows.Add((kind, new ScreenSectionsWindow(side)));
            }

            // Layout space and windowing-system space can differ — KWin maps every
            // XWayland output with one global factor — so the geometry comes from
            // the matching Avalonia screen, as the rulers do. Screens is readable
            // before the window is shown.
            var layoutBounds = source.Source.InPixel.Bounds;
            var screen = ScreenFinder.FromLayoutBounds(windows[0].Window.Screens, layoutBounds);

            var bounds = screen?.Bounds ?? new PixelRect(
                (int)layoutBounds.X, (int)layoutBounds.Y,
                (int)layoutBounds.Width, (int)layoutBounds.Height);
            var scaling = screen?.Scaling ?? source.Source.EffectiveDpi.Y / 96.0;
            var thickness = BandThickness * scaling;

            foreach (var (kind, window) in windows)
            {
                var (position, width, height) = kind switch
                {
                    BorderSideKind.Top =>
                        (new PixelPoint(bounds.X, bounds.Y), bounds.Width, thickness),
                    BorderSideKind.Bottom =>
                        (new PixelPoint(bounds.X, (int)(bounds.Bottom - thickness)), bounds.Width, thickness),
                    BorderSideKind.Left =>
                        (new PixelPoint(bounds.X, bounds.Y), thickness, (double)bounds.Height),
                    _ =>
                        (new PixelPoint((int)(bounds.Right - thickness), bounds.Y), thickness, (double)bounds.Height)
                };

                window.ShowAt(position, width, height, scaling);
                Overlay.Add(window);
            }
        }
    }

    public void Select(BorderSectionViewModel? section)
    {
        if (Selected != null) Selected.IsSelected = false;
        Selected = section;
        if (section != null) section.IsSelected = true;
    }
}