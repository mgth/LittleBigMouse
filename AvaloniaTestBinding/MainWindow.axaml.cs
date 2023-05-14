using Avalonia.Controls;
using System;
using System.Reactive;
using Avalonia.Media;
using ReactiveUI;

namespace AvaloniaTestBinding;

public class ViewModel : ReactiveObject
{
    public Stretch Stretch
    {
        get => _stretch;
        set => this.RaiseAndSetIfChanged(ref _stretch, value);
    }
    Stretch _stretch = Stretch.Uniform;

    public ViewModel()
    {
        TestCommand = ReactiveCommand.Create(RunTheThing);
    }

    public ReactiveCommand<Unit, Unit> TestCommand { get; }

    void RunTheThing()
    {
        Stretch = Stretch switch
        {
            Stretch.None => Stretch.Fill,
            Stretch.Fill => Stretch.Uniform,
            Stretch.Uniform => Stretch.UniformToFill,
            Stretch.UniformToFill => Stretch.None,
            _ => throw new ArgumentOutOfRangeException()
        };
    }

    public string Path => @"C:\Windows\Web\Wallpaper\Windows\img19.jpg";
}


public partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();

        var vm = DataContext = new ViewModel();


    }
}