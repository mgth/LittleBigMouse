/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Avalonia.Controls;
using Avalonia.Interactivity;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Plugins;
using MsBox.Avalonia;
using MsBox.Avalonia.Enums;
using System;
using System.Collections.Generic;
using System.IO.Compression;
using System.IO;
using System.Text;
using System.Threading.Tasks;
using Avalonia.Platform.Storage;
using System.Diagnostics;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// Logique d'interaction pour ControlGuiSizer.xaml
/// </summary>
public partial class LocationControlView : UserControl
    , IView<DefaultViewMode, LocationControlViewModel>, IMonitorsLayoutControlViewClass
{

    public LocationControlView()
    {
        InitializeComponent();
    }
    
    async void ImportJSon_Click(object? sender, RoutedEventArgs e)
    {
        try
        {
            var provider = TopLevel.GetTopLevel(this)?.StorageProvider;
            if (provider == null) return;

            var folder = await TryGetDocumentsFolderAsync(provider);

            var files = await provider.OpenFilePickerAsync(new FilePickerOpenOptions
            {
                AllowMultiple = false,
                FileTypeFilter = new FilePickerFileType[]{ new ("Config file")
                {
                    Patterns = new[] { "*.json" },
                    AppleUniformTypeIdentifiers = new[] { "public.json" },
                    MimeTypes = new[] { "application/json" }
                } },
                SuggestedStartLocation = folder
            });

            if (files.Count < 1) return;
            
            await using var stream = await files[0].OpenReadAsync();
            using var streamReader = new StreamReader(stream);
            var json = await streamReader.ReadToEndAsync();
                
            if (DataContext is not LocationControlViewModel vm) return;

            vm.ImportConfig(json);

        }
        catch (Exception ex)
        {
            await ShowErrorAsync("Import failed", ex);
        }
    }

    async void ExportJSon_Click(object? sender, RoutedEventArgs e)
    {
        if(DataContext is not LocationControlViewModel vm) return;

        var json = vm.ExportConfig();

        try
        {
            var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
            if (clipboard != null)
            {
                var data = new global::Avalonia.Input.DataTransfer();
                data.Add(global::Avalonia.Input.DataTransferItem.CreateText(json));
                await clipboard.SetDataAsync(data);
            }
        }
        catch
        {
            // a clipboard failure should not prevent exporting to a file
        }

        try
        {
            var provider = TopLevel.GetTopLevel(this)?.StorageProvider;
            if (provider == null) return;

            var folder = await TryGetDocumentsFolderAsync(provider);
            const string filename = "LittleBigMouse.Export.gz";

            var result = await provider.SaveFilePickerAsync(new FilePickerSaveOptions{
                DefaultExtension = ".export.gz" ,
                SuggestedFileName = filename,
                SuggestedStartLocation = folder
            });

            if (result == null) return;

            var path =  result.TryGetLocalPath();
            if (path == null) return;

            SaveConfig(json, path);
            OpenExplorerWithSelectedFile(path);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync("Export failed", ex);
        }
    }

    static async Task<IStorageFolder?> TryGetDocumentsFolderAsync(IStorageProvider provider)
    {
        try
        {
            return await provider.TryGetWellKnownFolderAsync(WellKnownFolder.Documents);
        }
        catch
        {
            // Documents may point to an unavailable location (e.g. a removed disk),
            // fall back to the picker's default location
            return null;
        }
    }

    static Task ShowErrorAsync(string caption, Exception ex)
        => MessageBoxManager
            .GetMessageBoxStandard(caption, ex.Message, ButtonEnum.Ok, Icon.Error, WindowStartupLocation.CenterOwner)
            .ShowAsync();

    static void SaveConfig(string txt, string path)
    {
        byte[] donnees = Encoding.UTF8.GetBytes(txt);

        using FileStream fichierSortie = File.Create(path);
        using GZipStream fluxGZip = new GZipStream(fichierSortie, CompressionMode.Compress);

        fluxGZip.Write(donnees, 0, donnees.Length);
    }

    static void OpenExplorerWithSelectedFile(string filePath)
    {
        if (!string.IsNullOrEmpty(filePath))
        {
            Process.Start("explorer.exe", $"/select,\"{filePath}\"");
        }
        else
        {
            Console.WriteLine("Invalid file path.");
        }
    }

}
