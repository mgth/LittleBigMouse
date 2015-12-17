using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using LbmScreenConfig;
using LittleBigMouse_Control;
using NotifyChange;

namespace LittleBigMouse_Control
{
    public class ScreenGuiControl<T> : ScreenGuiControl
    {
        protected ScreenGuiControl(Screen screen) : base(screen) {}
    }

    public class ScreenGuiControl : NotifyUserControl
    {
        public Screen Screen { get; }

        protected ScreenGuiControl(Screen screen)
        {
            Screen = screen;
            DataContext = this;
        }

        protected void OnKeyEnterUpdate(object sender, KeyEventArgs e)
        {
            if (e.Key != Key.Enter) return;

            TextBox tBox = (TextBox)sender;
            DependencyProperty prop = TextBox.TextProperty;

            BindingExpression binding = BindingOperations.GetBindingExpression(tBox, prop);
            binding?.UpdateSource();
        }

        public double WheelDelta(MouseWheelEventArgs e)
        {
            double delta = (e.Delta > 0) ? 1 : -1;
            if ((Keyboard.Modifiers & ModifierKeys.Control) != 0) delta /= 10;
            return delta;
        }
    }
}
