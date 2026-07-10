using Avalonia.Controls;
using Avalonia.Interactivity;
using Avalonia.Platform.Storage;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Plugin.Wallpaper.Avalonia;

public partial class WallpaperFrameView : UserControl,
    IView<WallpaperViewMode, WallpaperFrameViewModel>,
    IMonitorFrameContentViewClass
{
    public WallpaperFrameView()
    {
        InitializeComponent();
    }

    async Task<string?> PickImageAsync()
    {
        if (TopLevel.GetTopLevel(this) is not { } top) return null;

        var files = await top.StorageProvider.OpenFilePickerAsync(new FilePickerOpenOptions
        {
            Title = "Choose a wallpaper image",
            AllowMultiple = false,
            FileTypeFilter = [FilePickerFileTypes.ImageAll],
        });

        return files.Count > 0 ? files[0].TryGetLocalPath() : null;
    }

    async void OnPickImage(object? sender, RoutedEventArgs e)
    {
        if (DataContext is not WallpaperFrameViewModel vm) return;
        if (await PickImageAsync() is { } path) vm.SetImage(path);
    }

    async void OnPickSpanImage(object? sender, RoutedEventArgs e)
    {
        if (DataContext is not WallpaperFrameViewModel vm) return;
        if (await PickImageAsync() is { } path) vm.SetSpanImage(path);
    }
}
