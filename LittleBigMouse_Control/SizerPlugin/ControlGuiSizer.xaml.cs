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
using SystemColors1;
using LbmScreenConfig;

namespace LittleBigMouse_Control.SizerPlugin
{
    /// <summary>
    /// Logique d'interaction pour ControlGuiSizer.xaml
    /// </summary>
    public partial class ControlGuiSizer : ControlGui
    {

        public ControlGuiSizer() : base()
        {
            InitializeComponent();

            Config.PropertyChanged += Config_PropertyChanged;
            //foreach (ScreenGui screenGui in MainGui.AllScreenGuis)
            //{
            //    screenGui.SelectedChanged += OnSelectedChanged;
            //}
            DataContext = this;
        }

        private void Save()
        {
            Config.Save();
        }

        private void cmdOk_Click(object sender, RoutedEventArgs e)
        {
            cmdApply_Click(sender, e);
            MainGui.Close();
        }

        private void cmdApply_Click(object sender, RoutedEventArgs e)
        {
            Save();
            LittleBigMouseClient.Client.LoadAtStartup(Config.LoadAtStartup);
            LittleBigMouseClient.Client.LoadConfig();
            LittleBigMouseClient.Client.Start();
        }

        private void cmdCancel_Click(object sender, RoutedEventArgs e)
        {
            MainGui.Close();
        }

        private void OnClosing(object sender, CancelEventArgs e)
        {
            ShowRulers = false;
        }

        private void cmdUnload_Click(object sender, RoutedEventArgs e)
        {
            LittleBigMouseClient.Client.Quit();
        }

        private bool _showRulers = false;

        public bool ShowRulers
        {
            get { return _showRulers; }
            set
            {
                if (Change.SetProperty(ref _showRulers, value))
                {
                    if (_showRulers)
                    {
                        AddRuler(RulerSide.Left);
                        AddRuler(RulerSide.Right);
                        AddRuler(RulerSide.Top);
                        AddRuler(RulerSide.Bottom);

                        foreach (Ruler ruler in _rulers) ruler.Enabled = true;

                    }
                    else
                    {
                        foreach (Ruler sz in _rulers)
                        {
                            sz.Close();
                        }
                        _rulers.Clear();
                    }
                }
            }
        }

        private readonly List<Ruler> _rulers = new List<Ruler>();
        private bool _liveUpdate = false;

        private void AddRuler(RulerSide side)
        {
            if (Config.Selected == null) return;

            foreach (var sz in Config.AllScreens.Select(s => new Ruler(Config.Selected, s, side)))
            {
                _rulers.Add(sz);
            }
        }
        private void OnSelectedChanged(object s, bool selected)
        {
            if (!selected) return;

            if (ShowRulers)
            {
                ShowRulers = false;
                ShowRulers = true;
            }
        }

        public bool LiveUpdate
        {
            get { return _liveUpdate; }
            set
            {
                if (Change.SetProperty(ref _liveUpdate, value) && value)
                    ActivateConfig();
                else
                {
                    LittleBigMouseClient.Client.LoadConfig();
                }
            }
        }

        public void ActivateConfig()
        {
            if (LiveUpdate)
            {
                Save();
                LittleBigMouseClient.Client.LoadConfig();
            }

        }
        private void Config_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            ActivateConfig();
        }

        private void cmdColors_Click(object sender, RoutedEventArgs e)
        {
            new ColorsWindow().Show();
        }
    }
}