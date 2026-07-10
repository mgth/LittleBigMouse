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
using Avalonia.Controls;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

/// <summary>
/// Borderless topmost window covering a single screen edge, hosting one ruler.
/// One window per edge (instead of one fullscreen window per screen) so the
/// screen center is never covered and clicks reach the applications below,
/// without any OS-specific window-region cutting.
/// </summary>
public partial class RulerWindow : Window
{
    PixelPoint _position;
    double _pixelWidth;
    double _pixelHeight;

    public RulerWindow()
    {
        InitializeComponent();
    }

    public RulerWindow(RulerViewModel viewModel) : this()
    {
        DataContext = viewModel;

        // The actual scaling is only known once the window is mapped on its
        // screen; the ShowAt value is just a hint that may differ from what
        // the windowing system applies.
        Opened += (_, _) => FitToPixels();
    }

    /// <summary>
    /// Position and size the window in windowing-system pixels, then show it.
    /// </summary>
    public void ShowAt(PixelPoint position, double pixelWidth, double pixelHeight, double scalingHint)
    {
        _position = position;
        _pixelWidth = pixelWidth;
        _pixelHeight = pixelHeight;

        Position = position;
        SetDipSize(scalingHint > 0 ? scalingHint : 1.0);
        Show();
    }

    void FitToPixels()
    {
        SetDipSize(DesktopScaling);
        Position = _position;
    }

    void SetDipSize(double scaling)
    {
        Width = _pixelWidth / scaling;
        Height = _pixelHeight / scaling;
    }
}
