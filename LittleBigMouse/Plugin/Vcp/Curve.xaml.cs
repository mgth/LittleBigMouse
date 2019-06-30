/*
  LittleBigMouse.Plugin.Vcp
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Collections.Generic;
using System.ComponentModel;
using System.Windows.Controls;
using HLab.Notify;

namespace LittleBigMouse.Plugin.Vcp
{
    /// <summary>
    /// Logique d'interaction pour Curve.xaml
    /// </summary>
    public partial class Curve : UserControl
    {
        private IList<double> _points;
        public Curve()
        {
            InitializeComponent();
            DataContext = this;
        }
       // private PlotModel _plotModel = new PlotModel();
        //public PlotModel PlotModel
        //{
        //    get { return _plotModel; }
        //    set { Change.Set(ref _plotModel, value); }
        //}

        public void Refresh()
        {
            //PlotView.InvalidateVisual();
        }

    }
}
