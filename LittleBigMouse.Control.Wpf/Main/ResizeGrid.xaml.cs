using HLab.Base.Wpf;
using HLab.Sys.Windows.Monitors;

using System;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Markup;
using System.Windows.Media;

namespace LittleBigMouse.Control.Main;

using H = DependencyHelper<ResizeGrid>;

/// <summary>
/// Logique d'interaction pour ResizeGrid.xaml
/// </summary>
/// 
public enum ResizeDirection
{
    Left = 1,
    Right = 2,
    Top = 3,
    TopLeft = 4,
    TopRight = 5,
    Bottom = 6,
    BottomLeft = 7,
    BottomRight = 8,
}

[ContentProperty("NestedContent")]
public partial class ResizeGrid : UserControl
{
    public ResizeGrid()
    {
        InitializeComponent();

        this.Loaded += ResizeGrid_Loaded;
    }

    Window Window => Window.GetWindow(this);

    void ResizeGrid_Loaded(object sender, RoutedEventArgs e)
    {
        if(Window.IsInitialized) InitializeWindowSource(sender,e);
        Window.SourceInitialized += InitializeWindowSource;
    }

    public object NestedContent
    {
        get => GetValue(NestedContentProperty);
        set => SetValue(NestedContentProperty, value);
    }
    public static readonly DependencyProperty NestedContentProperty = H.Property<object>().Register();

    [DllImport("user32.dll", CharSet = CharSet.Auto)]
    static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);

    const int WmSyscommand = 0x112;
    HwndSource _hwndSource;

    void ResizeWindow(ResizeDirection direction)
    {
        SendMessage(_hwndSource.Handle, WmSyscommand, (IntPtr)(61440 + direction), IntPtr.Zero);
    }

    void InitializeWindowSource(object sender, EventArgs e)
    {
        _hwndSource = PresentationSource.FromVisual((Visual)sender) as HwndSource;
        _hwndSource?.AddHook(WndProc);
    }

    IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            switch (msg)
            {
                case (int)WindowMessage.WM_NCHITTEST:
                    try
                    {
                        int x = lParam.ToInt32() & 0xffff;
                        int y = lParam.ToInt32() >> 16;
                        var rect = new Rect(MaximizeButton.PointToScreen(new Point()), new Size(MaximizeButton.Width, MaximizeButton.Height));
                        if (rect.Contains(new Point(x, y)))
                        {
                            handled = true;
                        }
                        return new IntPtr(9/*HTMAXBUTTON*/);
                    }
                    catch (OverflowException)
                    {
                        handled = true;
                    }
                    break;

                case (int)WindowMessage.WM_NCLBUTTONDOWN:
                     handled = true;
                    break;
                default:    
                break;
            }

            return IntPtr.Zero;
        }

    void Border_MouseDown(object sender, MouseButtonEventArgs e)
    {
        if (sender is not FrameworkElement clicked) return;
        if (clicked.Tag is not ResizeDirection dir) return;

        ResizeWindow(dir);
    }

    public void DragWindow(object sender, MouseEventArgs mouseEventArgs)
    {
        if (mouseEventArgs.LeftButton == MouseButtonState.Released) return;
        Window.DragMove();
    }

    void UserControl_MouseDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ClickCount == 2) ToggleMaximize();
        else
        {
//                this.EnableBlur(false);
            DragWindow(sender,e);
        }
    }

    void ButtonMaximize_OnClick(object sender, RoutedEventArgs e)
    {
        ToggleMaximize();
    }

    void ToggleMaximize()
    {
           Window.WindowState = Window.WindowState==WindowState.Maximized?WindowState.Normal : WindowState.Maximized;
    }

    void ButtonMinimize_OnClick(object sender, RoutedEventArgs e)
    {
        Window.WindowState = WindowState.Minimized;
    }

    void ButtonClose_Click(object sender, RoutedEventArgs e)
    {
        Window.Close();
    }
}
