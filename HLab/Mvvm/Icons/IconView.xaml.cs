using System.Windows;
using System.Windows.Controls;

namespace HLab.Mvvm.Icons
{
    /// <summary>
    /// Logique d'interaction pour Icon.xaml
    /// </summary>
    public partial class IconView : UserControl
    {
        public IconView()
        {
            InitializeComponent();
        }

        public static readonly DependencyProperty SourceNameProperty = DependencyProperty.Register(
            nameof(SourceName),
            typeof(string),
            typeof(IconView),
            new FrameworkPropertyMetadata(OnSourceNameChanged));

        public string SourceName
        {
            get => (string)GetValue(SourceNameProperty); set => SetValue(SourceNameProperty, value);
        }

        private static void OnSourceNameChanged(DependencyObject source,
                DependencyPropertyChangedEventArgs e)
        {
            var s = (IconView)source;

            s.Update(s.AssemblyName, (string)e.NewValue);
        }

        public static readonly DependencyProperty AssemblyNameProperty = DependencyProperty.Register(
            nameof(AssemblyName),
            typeof(string),
            typeof(IconView),
            new FrameworkPropertyMetadata(OnAssemblyNameChanged));

        public string AssemblyName
        {
            get => (string)GetValue(AssemblyNameProperty); set => SetValue(AssemblyNameProperty, value);
        }

        private static void OnAssemblyNameChanged(DependencyObject source,
                DependencyPropertyChangedEventArgs e)
        {
            var s = (IconView)source;

            s.Update((string)e.NewValue, s.SourceName);
        }


        public void Update(string assembly, string name)
        {
            if (string.IsNullOrWhiteSpace(assembly)) return;
            if (string.IsNullOrWhiteSpace(name)) return;
            Viewbox.Child = IconService.D.GetIcon(assembly, name);
        }
    }
}
