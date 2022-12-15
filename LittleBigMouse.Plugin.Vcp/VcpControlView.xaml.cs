/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.

    LittleBigMouse.Plugin.Vcp is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Windows;
using System.Windows.Controls;

using HLab.Mvvm.Annotations;

using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Plugin.Vcp
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class VcpControlView : UserControl, IView<ViewModeScreenVcp, VcpScreenViewModel>
    {
        readonly ILittleBigMouseClientService _service;

        public VcpControlView(ILittleBigMouseClientService service)
        {
            _service = service;
            InitializeComponent();
        }

        void Save()
        {
 //           Model.Save();
        }


        void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            //LittleBigMouseClient.Client.LoadAtStartup(Model.LoadAtStartup);
            //_service.Start();
        }

        void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
           // MainGui.Close();
        }


        void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            _service.Quit();
        }

        void ButtonBase_OnClick(object sender, RoutedEventArgs routedEventArgs)
        {
            ;
        }


    }
}