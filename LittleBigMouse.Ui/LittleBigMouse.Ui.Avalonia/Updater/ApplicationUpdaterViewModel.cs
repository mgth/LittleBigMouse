using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Text.Json.Nodes;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Input;
using Avalonia.Controls;
using Avalonia.Platform;
using Avalonia.Threading;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.Plugins;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Updater;


public class ApplicationUpdaterViewModel : ViewModel, IApplicationUpdater
{
    IMvvmService _mvvm;
    public ApplicationUpdaterViewModel(IMvvmService mvvm)
    {
        _mvvm = mvvm;
        _newVersionFound = this.WhenAnyValue(
            e => e.CurrentVersion,
            e => e.NewVersion,
            (o, n) => NewVersion > CurrentVersion)
            .ToProperty(this, e => e.NewVersionFound);

        UpdateCommand = ReactiveCommand.Create(Update, this.WhenAnyValue(e => e.NewVersionFound));
    }
    public object MainIcon { get; } = new WindowIcon(AssetLoader.Open(new Uri("avares://LittleBigMouse.Ui.Avalonia/Assets/lbm-logo.ico")));

    public void Show()
    {
        var view = new ApplicationUpdaterView
        {
            DataContext = this
        };
        view.ShowDialog(null);
    }

    public double Progress
    {
        get => _progress;
        set => this.RaiseAndSetIfChanged(ref _progress, value);
    }
    double _progress;

    public bool Updated
    {
        get => _updated;
        set => this.RaiseAndSetIfChanged(ref _updated, value);
    }
    bool _updated;

    public Version CurrentVersion { get; } = GetVersion();

    static Version GetVersion()
    {
        try
        {
            var file = Process.GetCurrentProcess().MainModule?.FileName;
            var v = FileVersionInfo.GetVersionInfo(file).FileVersion;
            return Version.TryParse(v, out var version) ? version : new Version();
        }
        catch (FileNotFoundException)
        {
            return new Version();
        }
    }

    public Version NewVersion
    {
        get => _newVersion;
        set => this.RaiseAndSetIfChanged(ref _newVersion, value);
    }
    Version _newVersion = new Version();

    public string Url
    {
        get => _url;
        set => this.RaiseAndSetIfChanged(ref _url, value);
    }
    string _url;

    public string FileName
    {
        get => _fileName;
        set => this.RaiseAndSetIfChanged(ref _fileName, value);
    }
    string _fileName;

    public string Message
    {
        get => _message;
        set => this.RaiseAndSetIfChanged(ref _message, value);
    }
    string _message;

    public ICommand UpdateCommand { get; }

    public bool NewVersionFound => _newVersionFound.Value;
    readonly ObservableAsPropertyHelper<bool> _newVersionFound;

    void Update()
    {
        var filename = Url.Split('/').Last(); //FileName.Replace("{version}", NewVersion.ToString());
        var path = Path.GetTempPath() + filename;

        var thread = new Thread(() =>
        {
            var client = new WebClient();
            client.DownloadProgressChanged += DownloadProgressChanged;
            client.DownloadFileCompleted += DownloadFileCompleted;
            client.DownloadFileAsync(new Uri(Url), path);
        });
        thread.Start();
    }

    void DownloadProgressChanged(object sender, DownloadProgressChangedEventArgs e)
    {
        var bytesIn = double.Parse(e.BytesReceived.ToString());
        var totalBytes = double.Parse(e.TotalBytesToReceive.ToString());
        Progress = bytesIn / totalBytes * 100;
    }

    void DownloadFileCompleted(object sender, AsyncCompletedEventArgs e)
    {
        RunUpdate();
    }

    void RunUpdate()
    {
        var filename = Url.Split('/').Last(); //FileName.Replace("{version}", NewVersion.ToString());
        var path = Path.GetTempPath() + filename;
        var startInfo = new ProcessStartInfo(path) { Verb = "runas" };
        try
        {
            Process.Start(startInfo);
            Updated = true;

            Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
        }
        catch (Win32Exception)
        {
            Message = "Update failed";
        }
        catch (WebException)
        {
            Message = "Download failed";
        }
    }

    public async Task CheckVersion()
    {
        using var client = new HttpClient();

        client.DefaultRequestHeaders.Add("User-Agent", "LittleBigMouse");
        var r = await client.GetAsync("https://api.github.com/repos/Mgth/LittleBigMouse/releases");
        var json = JsonNode.Parse(await r.Content.ReadAsStringAsync());

        if (json is not JsonArray releases) return;

        // Pick the newest published, non-prerelease release that has a downloadable asset
        // and a parseable version. Read the version from the release name (historically a
        // bare "5.2.3.0") but fall back to the tag ("v5.3.0"), and tolerate a "v" prefix,
        // so a non-numeric release title can no longer break updates (was showing "0.0").
        foreach (var release in releases)
        {
            if (release is null) continue;
            if (release["draft"]?.GetValue<bool>() == true) continue;
            if (release["prerelease"]?.GetValue<bool>() == true) continue;

            if (release["assets"] is not JsonArray assets || assets.Count == 0) continue;

            if (!TryParseVersion(release["name"]?.GetValue<string>(), out var onlineVersion)
                && !TryParseVersion(release["tag_name"]?.GetValue<string>(), out onlineVersion))
                continue;

            NewVersion = onlineVersion;
            Message = release["body"]?.ToString() ?? "";
            Url = assets[0]?["browser_download_url"]?.ToString() ?? "";
            return;
        }
    }

    static bool TryParseVersion(string? text, out Version version)
    {
        version = new Version();
        if (string.IsNullOrWhiteSpace(text)) return false;
        text = text.Trim();
        if (text[0] is 'v' or 'V') text = text[1..];
        return Version.TryParse(text, out version!);
    }
    
    public async Task CheckUpdateAsync(bool show)
    {
        if(show)
        {
            var updaterView = new ApplicationUpdaterView
            {
                DataContext = this
            };
            updaterView.Show();
        }

        await CheckVersion();

        if (NewVersionFound)
        {
            if(!show)
            {
                var updaterView = new ApplicationUpdaterView
                {
                    DataContext = this
                };
                updaterView.Show();
            }

            if (Updated)
            {
                //Application.Current.Shutdown();
                return;
            }
        }
    }
    
}