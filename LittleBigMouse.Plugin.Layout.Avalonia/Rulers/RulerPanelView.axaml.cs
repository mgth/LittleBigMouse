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

using Avalonia.Controls;
using Avalonia.Media;
using HLab.Sys.Windows.API;

namespace LittleBigMouse.Plugin.Layout.Avalonia.Rulers;

/// <summary>
/// Logique d'interaction pour ScreenPanel.xaml
/// </summary>
public partial class RulerPanelView : Window
{
    public const int GWL_EXSTYLE = -20;
    public const int WS_EX_LAYERED = 0x80000;
    public const int WS_EX_TRANSPARENT = 0x20;
    public RulerPanelView()
    {
        InitializeComponent();

        //WinUser.SetWindowLong(handle.Handle,
        //    WinUser.WindowLongFlags.ExStyle, GWL_EXSTYLE | WS_EX_LAYERED | WS_EX_TRANSPARENT);//)
        //WinUser.SetLayeredWindowAttributes(handle.Handle, 0, 255, 0x2);

    }


    public override void Render(DrawingContext context)
    {
        var handle = TryGetPlatformHandle();
        if (handle != null)
        {
            Cut(handle.Handle);
        }
        base.Render(context);
    }
    

    void Cut(IntPtr handle)
    {
        //if (this.DataContext is not RulerPanelViewModel viewModel) return;
        //var size = viewModel.DrawOn.Source.InPixel;


        WinUser.GetWindowRect(handle,out var r);

        var size = (int)((r.Width / TopRuler.Bounds.Width) * TopRuler.Bounds.Height);

        var win = WinUser.CreateRectRgn(0, 0, r.Width, r.Height);

        var inside = WinUser.CreateRectRgn(size, size, r.Width - size,r.Height - size);

        WinGdi.CombineRgn(win, win, inside, WinGdi.CombineRgnStyles.Diff);
        WinUser.SetWindowRgn(handle, win , true);

        WinGdi.DeleteDC(win);
        WinGdi.DeleteDC(inside);
    }


}