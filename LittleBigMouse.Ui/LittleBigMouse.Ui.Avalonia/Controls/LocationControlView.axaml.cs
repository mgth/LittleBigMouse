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
    
    /// <summary>
    /// Loads an exported layout for on-screen inspection (debug tool):
    /// the layout is only displayed, it never drives the mouse engine.
    /// </summary>
    async void OpenVirtualLayout_Click(object? sender, RoutedEventArgs e)
    {
        try
        {
            var provider = TopLevel.GetTopLevel(this)?.StorageProvider;
            if (provider == null) return;

            var folder = await TryGetDocumentsFolderAsync(provider);

            var files = await provider.OpenFilePickerAsync(new FilePickerOpenOptions
            {
                AllowMultiple = false,
                FileTypeFilter = new FilePickerFileType[]{ new ("Layout export")
                {
                    Patterns = new[] { "*.json", "*.gz" },
                    AppleUniformTypeIdentifiers = new[] { "public.json", "org.gnu.gnu-zip-archive" },
                    MimeTypes = new[] { "application/json", "application/gzip" }
                } },
                SuggestedStartLocation = folder
            });

            if (files.Count < 1) return;

            await using var stream = await files[0].OpenReadAsync();
            var json = await ReadConfigAsync(stream);

            if (DataContext is not LocationControlViewModel vm) return;

            vm.OpenVirtualLayout(json);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync("Open virtual layout failed", ex);
        }
    }

    async void ExportLayout_Click(object? sender, RoutedEventArgs e)
    {
        if(DataContext is not LocationControlViewModel vm) return;

        try
        {
            var json = vm.ExportConfig();

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

    async void CopyLayout_Click(object? sender, RoutedEventArgs e)
    {
        if(DataContext is not LocationControlViewModel vm) return;

        try
        {
            var json = vm.ExportConfig();

            var clipboard = TopLevel.GetTopLevel(this)?.Clipboard;
            if (clipboard == null) return;

            var data = new global::Avalonia.Input.DataTransfer();
            data.Add(global::Avalonia.Input.DataTransferItem.CreateText(json));
            await clipboard.SetDataAsync(data);
        }
        catch (Exception ex)
        {
            await ShowErrorAsync("Copy failed", ex);
        }
    }

    /// <summary>
    /// Reads an export as text, transparently decompressing gzip files
    /// (the export file is gzipped, the clipboard export is plain json).
    /// </summary>
    static async Task<string> ReadConfigAsync(Stream stream)
    {
        using var memory = new MemoryStream();
        await stream.CopyToAsync(memory);

        var bytes = memory.ToArray();

        if (bytes.Length > 2 && bytes[0] == 0x1f && bytes[1] == 0x8b)
        {
            using var gzip = new GZipStream(new MemoryStream(bytes), CompressionMode.Decompress);
            using var reader = new StreamReader(gzip, Encoding.UTF8);
            return await reader.ReadToEndAsync();
        }

        return Encoding.UTF8.GetString(bytes);
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
