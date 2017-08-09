using System.Collections.Generic;
using System.ComponentModel;
using System.Windows.Controls;
using Erp.Notify;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour Curve.xaml
    /// </summary>
    public partial class Curve : UserControl, INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

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
