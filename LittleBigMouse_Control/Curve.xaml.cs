using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;
using LbmScreenConfig;
using OxyPlot;
using OxyPlot.Wpf;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour Curve.xaml
    /// </summary>
    public partial class Curve : UserControl, INotifyPropertyChanged
    {
        protected readonly PropertyChangedHelper Change;
        private IList<DataPoint> _points;
        public event PropertyChangedEventHandler PropertyChanged { add { Change.Add(this, value); } remove { Change.Remove(value); } }
        public Curve()
        {
            Change = new PropertyChangedHelper(this);
            InitializeComponent();
            DataContext = this;
        }
        private PlotModel _plotModel = new PlotModel();
        public PlotModel PlotModel
        {
            get { return _plotModel; }
            set { Change.SetProperty(ref _plotModel, value); }
        }

        public void Refresh()
        {
            PlotView.InvalidatePlot();
        }

    }
}
