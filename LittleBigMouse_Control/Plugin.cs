using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    public abstract class Plugin<T> : Plugin where T : Plugin, new()
    {
        private static T _instance;
        public static T Instance => _instance ?? (_instance = new T());
    }

    public abstract class Plugin : INotifyPropertyChanged
    {
        // PropertyChanged Handling
        protected readonly PropertyChangedHelper Change;
        public event PropertyChangedEventHandler PropertyChanged { add { Change.Add(this, value); } remove { Change.Remove(value); } }

        public MainGui MainGui => MainGui.Instance;

        protected Plugin()
        {
            Change = new PropertyChangedHelper(this);
        }

        public abstract bool Init();

        public void AddButton(IPluginButton plugin)
        {
            ToggleButton tb = new ToggleButton
            {
                Content = plugin.Caption,
                DataContext = plugin,
            };

            Binding binding = new Binding
            {
                Mode = BindingMode.TwoWay,
                Path = new PropertyPath("IsActivated"),
            };

            tb.SetBinding(ToggleButton.IsCheckedProperty, binding);

            MainGui.ButtonPanel.Children.Add(tb);
        }

    }
}
