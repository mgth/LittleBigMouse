using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Data;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    class MainViewModel : ViewModel
    {
        public override Type ViewType => typeof (MainView);

        public MainViewModel()
        {
            Plugins.CollectionChanged += Plugins_CollectionChanged;
        }

        private void Plugins_CollectionChanged(object sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e)
        {
            if (e.NewItems == null) return;
            foreach (Plugin plugin in e.NewItems)
            {
                plugin.MainViewModel = this;
                plugin.Init();
            }
        }

        public readonly ObservableCollection<Plugin> Plugins = new ObservableCollection<Plugin>();

        public static DependencyProperty ConfigProperty = DependencyProperty.Register(nameof(Config),typeof(ScreenConfig),typeof(MainViewModel));

        public static DependencyProperty ControlProperty = DependencyProperty.Register(nameof(Control),typeof(ViewModel),typeof(MainViewModel));

        public static DependencyProperty PresenterProperty = DependencyProperty.Register(
            nameof(Presenter), 
            typeof(ViewModel), 
            typeof(MainViewModel), 
            new FrameworkPropertyMetadata(
                null,
                new PropertyChangedCallback(PresenterChanged)));

        private static void PresenterChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            PresenterViewModel presenter = (e.NewValue as PresenterViewModel);
            if (presenter != null)
                presenter.MainViewModel = d as MainViewModel;
        }

        //private GetScreenControlViewModelDelegate _getScreenControlView;


        public ScreenConfig Config
        {
            get { return (ScreenConfig)GetValue(ConfigProperty); }
            set { SetValue(ConfigProperty, value); }
        }


        public ViewModel Control
        {
            get { return (ViewModel)GetValue(ControlProperty); }
            set { SetValue(ControlProperty,value);}
        }
        public PresenterViewModel Presenter
        {
            get { return (PresenterViewModel)GetValue(PresenterProperty); }
            set { SetValue(PresenterProperty, value); }
        }

        public StackPanel ButtonPanel { get; } = new StackPanel
        {
            Orientation = Orientation.Horizontal,
        };
       

        public void AddButton(IPluginButton plugin)
        {
            ToggleButton tb = new ToggleButton
            {
                //Style = (Style)MainView.Resources["ButtonStyle"],
                Content = plugin.Caption,
                DataContext = plugin,
            };

            Binding binding = new Binding
            {
                Mode = BindingMode.TwoWay,
                Path = new PropertyPath("IsActivated"),
            };

            tb.SetBinding(ToggleButton.IsCheckedProperty, binding);

            ButtonPanel.Children.Add(tb);
        }
    }
}
