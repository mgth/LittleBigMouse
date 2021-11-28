using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using HLab.Base;
using HLab.Mvvm.Icons;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.Control.Main
{
    using H = DependencyHelper<ButtonPanel>;
    /// <summary>
    /// Logique d'interaction pour ButtonPanel.xaml
    /// </summary>
    public partial class ButtonPanel : UserControl
    {
        public ButtonPanel()
        {
            InitializeComponent();
        }

        public static readonly DependencyProperty CommandsProperty = H.Property<IEnumerable<ICommand>>()
            .OnChange((c,a) => c.OnChange(a))
            .Register();

        private void OnChange(DependencyPropertyChangedEventArgs<IEnumerable<ICommand>> a)
        {
            if (a.OldValue is ObservableCollection<ICommand> oldValue)
            {
                oldValue.CollectionChanged -= ValueOnCollectionChanged;
                ValueOnCollectionChanged(null,new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Remove,oldValue));
            }

            if (a.NewValue is ObservableCollection<ICommand> newValue)
            {
                newValue.CollectionChanged += ValueOnCollectionChanged;
                ValueOnCollectionChanged(null,new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,newValue));
            }
        }

        private readonly Dictionary<ICommand,FrameworkElement> _buttons = new Dictionary<ICommand, FrameworkElement>();

        private void ValueOnCollectionChanged(object? sender, NotifyCollectionChangedEventArgs e)
        {
            switch (e.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    
                    foreach (var item in e.NewItems.OfType<ICommand>())
                    {
                        var button = GetButton(item);
                        _buttons.Add(item,button);
                        StackPanel.Children.Add(button);
                    }
                    break;

                case NotifyCollectionChangedAction.Remove:
                    foreach (var item in e.OldItems.OfType<ICommand>())
                    {
                        if (_buttons.TryGetValue(item, out var button))
                        {
                            _buttons.Remove(item);
                            StackPanel.Children.Remove(button);
                        }
                    }
                    break;
                case NotifyCollectionChangedAction.Replace:
                    break;
                case NotifyCollectionChangedAction.Move:
                    break;
                case NotifyCollectionChangedAction.Reset:
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }

        private FrameworkElement GetButton(ICommand cmd)
        {
            if (cmd is INotifyCommand ncmd)
            {
                var tb =  new ToggleButton
                {
                    ToolTip = ncmd.ToolTipText,
                    Height = 40,
                    Width = 40,
                    Margin = new Thickness(5),
                    Padding = new Thickness(5),
                    Content = new IconView {Path = ncmd.IconPath},
                };
                tb.Checked += (sender, args) =>
                {
                    foreach (var other in StackPanel.Children.OfType<ToggleButton>().Where(e => !ReferenceEquals(e,tb)))
                    {
                        other.IsChecked = false;
                    }

                    cmd.Execute(true);
                };

                tb.Unchecked += (sender, args) =>
                {
                    cmd.Execute(false);
                };

                return tb;
            }
            throw new InvalidCastException();
        }

        public IEnumerable<ICommand> Commands
        {
            get => (IEnumerable<ICommand>) GetValue(CommandsProperty);
            set => SetValue(CommandsProperty, value);
        } 
    }
}
