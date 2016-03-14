using System.Collections.Generic;
using System.ComponentModel;
using System.Windows.Controls;
using NotifyChange;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour Curve.xaml
    /// </summary>
    public partial class Curve : UserControl, INotifyPropertyChanged
    {
        protected readonly NotifierHelper Notify;
        private IList<double> _points;
        public event PropertyChangedEventHandler PropertyChanged { add { Notify.Add(value); } remove { Notify.Remove(value); } }
        public Curve()
        {
            Notify = new NotifierHelper(this);
            InitializeComponent();
            DataContext = this;
        }
       // private PlotModel _plotModel = new PlotModel();
        //public PlotModel PlotModel
        //{
        //    get { return _plotModel; }
        //    set { Change.SetProperty(ref _plotModel, value); }
        //}

        public void Refresh()
        {
            //PlotView.InvalidateVisual();
        }

    }
}
