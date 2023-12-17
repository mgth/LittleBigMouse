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
using Avalonia.Threading;
using HLab.Mvvm.ReactiveUI;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Updater;

public class ApplicationUpdaterViewModel : ViewModel
{
    public ApplicationUpdaterViewModel()
    {
        _newVersionFound = this.WhenAnyValue(
            e => e.CurrentVersion,
            e => e.NewVersion,
            (o, n) => NewVersion > CurrentVersion)
            .ToProperty(this, e => e.NewVersionFound);

        UpdateCommand = ReactiveCommand.Create(Update, this.WhenAnyValue(e => e.NewVersionFound));
    }

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
        var json = JsonNode.Parse(r.Content.ReadAsStringAsync().Result);

        var name = json[0]["name"].GetValue<string>();

        Message = json[0]["body"].ToString();

        Url = json[0]["assets"][0]["browser_download_url"].ToString();

        if (Version.TryParse(name, out var onlineVersion))
        {
            NewVersion = onlineVersion;
        }
    }
}