#nullable enable
using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
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
    static readonly HttpClient ApiHttp = CreateHttpClient(TimeSpan.FromSeconds(8));
    static readonly HttpClient DownloadHttp = CreateHttpClient(TimeSpan.FromMinutes(10));
    readonly ObservableAsPropertyHelper<bool> _newVersionFound;
    ReleaseUpdateInfo? _release;
    string? _downloadedPath;

    public ApplicationUpdaterViewModel(IMvvmService mvvm)
    {
        _ = mvvm;
        _newVersionFound = this.WhenAnyValue(
                updater => updater.CurrentVersion,
                updater => updater.NewVersion,
                (_, _) => NewVersion > CurrentVersion)
            .ToProperty(this, updater => updater.NewVersionFound);
        UpdateCommand = ReactiveCommand.CreateFromTask(UpdateAsync,
            this.WhenAnyValue(updater => updater.NewVersionFound));
    }

    public object MainIcon { get; } = new WindowIcon(AssetLoader.Open(
        new Uri("avares://LittleBigMouse.Ui.Avalonia/Assets/lbm-logo.ico")));
    public ICommand UpdateCommand { get; }
    public Version CurrentVersion { get; } = GetVersion();
    public bool NewVersionFound => _newVersionFound.Value;

    public double Progress { get => _progress; set => this.RaiseAndSetIfChanged(ref _progress, value); }
    double _progress;
    public bool Updated { get => _updated; set => this.RaiseAndSetIfChanged(ref _updated, value); }
    bool _updated;
    public Version NewVersion { get => _newVersion; set => this.RaiseAndSetIfChanged(ref _newVersion, value); }
    Version _newVersion = new();
    public string Url { get => _url; set => this.RaiseAndSetIfChanged(ref _url, value); }
    string _url = "";
    public string FileName { get => _fileName; set => this.RaiseAndSetIfChanged(ref _fileName, value); }
    string _fileName = "";
    public string Message { get => _message; set => this.RaiseAndSetIfChanged(ref _message, value); }
    string _message = "";

    public void Show()
    {
        var view = new ApplicationUpdaterView { DataContext = this };
        view.ShowDialog(null);
    }

    public async Task CheckUpdateAsync(bool show)
    {
        if (show) new ApplicationUpdaterView { DataContext = this }.Show();
        try
        {
            using var response = await ApiHttp.GetAsync(
                "https://api.github.com/repos/Mgth/LittleBigMouse/releases",
                HttpCompletionOption.ResponseHeadersRead).ConfigureAwait(true);
            response.EnsureSuccessStatusCode();
            var document = JsonNode.Parse(await response.Content.ReadAsStringAsync()
                .ConfigureAwait(true));
            _release = ReleaseUpdateSecurity.SelectNewest(document);
            if (_release is null) return;
            NewVersion = _release.Version;
            Message = _release.Message;
            Url = _release.DownloadUri.AbsoluteUri;
            FileName = _release.FileName;
        }
        catch (Exception error) when (error is HttpRequestException
                                      or TaskCanceledException
                                      or System.Text.Json.JsonException
                                      or InvalidOperationException
                                      or FormatException)
        {
            Message = $"Update check unavailable: {error.Message}";
            return;
        }

        if (NewVersionFound && !show)
            new ApplicationUpdaterView { DataContext = this }.Show();
    }

    async Task UpdateAsync()
    {
        if (_release is null || _release.Version <= CurrentVersion) return;
        var directory = Path.Combine(Path.GetTempPath(), "LittleBigMouse", "updates",
            Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        var path = Path.Combine(directory, _release.FileName);
        try
        {
            using var response = await DownloadHttp.GetAsync(_release.DownloadUri,
                HttpCompletionOption.ResponseHeadersRead);
            response.EnsureSuccessStatusCode();
            if (response.Content.Headers.ContentLength is { } length
                && length != _release.Size)
                throw new InvalidDataException("The installer size does not match release metadata.");

            await using var source = await response.Content.ReadAsStreamAsync();
            await using (var destination = new FileStream(path, FileMode.CreateNew,
                             FileAccess.Write, FileShare.None, 81920, useAsync: true))
            {
                var buffer = new byte[81920];
                long written = 0;
                while (true)
                {
                    var count = await source.ReadAsync(buffer);
                    if (count == 0) break;
                    await destination.WriteAsync(buffer.AsMemory(0, count));
                    written += count;
                    Progress = Math.Min(100, 100d * written / _release.Size);
                    if (written > _release.Size)
                        throw new InvalidDataException("The installer is larger than release metadata.");
                }
                await destination.FlushAsync();
                if (written != _release.Size)
                    throw new InvalidDataException("The installer is incomplete.");
            }

            if (!ReleaseUpdateSecurity.VerifySha256(path, _release.Sha256))
                throw new InvalidDataException("The installer SHA-256 digest is invalid.");
            if (!await Task.Run(() => AuthenticodeVerifier.IsTrustedPublisher(
                    path, ReleaseUpdateSecurity.ExpectedPublisher)))
                throw new InvalidDataException(
                    $"The installer is not signed by {ReleaseUpdateSecurity.ExpectedPublisher}.");

            _downloadedPath = path;
            await Dispatcher.UIThread.InvokeAsync(RunVerifiedUpdate);
        }
        catch (Exception error) when (error is HttpRequestException
                                      or TaskCanceledException
                                      or IOException
                                      or UnauthorizedAccessException
                                      or Win32Exception)
        {
            TryDelete(path);
            Message = $"Update failed safely: {error.Message}";
        }
    }

    void RunVerifiedUpdate()
    {
        var path = _downloadedPath;
        if (path is null
            || _release is null
            || !ReleaseUpdateSecurity.VerifySha256(path, _release.Sha256)
            || !AuthenticodeVerifier.IsTrustedPublisher(
                path, ReleaseUpdateSecurity.ExpectedPublisher))
        {
            Message = "Update blocked because installer verification failed.";
            return;
        }

        try
        {
            Process.Start(new ProcessStartInfo(path)
            {
                Verb = "runas",
                UseShellExecute = true,
            });
            Updated = true;
            Dispatcher.UIThread.BeginInvokeShutdown(DispatcherPriority.Normal);
        }
        catch (Win32Exception error)
        {
            Message = $"Update was not started: {error.Message}";
        }
    }

    static HttpClient CreateHttpClient(TimeSpan timeout)
    {
        var client = new HttpClient { Timeout = timeout };
        client.DefaultRequestHeaders.UserAgent.ParseAdd("LittleBigMouse-Updater/1.0");
        client.DefaultRequestHeaders.Accept.ParseAdd("application/vnd.github+json");
        client.DefaultRequestHeaders.Add("X-GitHub-Api-Version", "2022-11-28");
        return client;
    }

    static Version GetVersion()
    {
        try
        {
            var file = Process.GetCurrentProcess().MainModule?.FileName;
            if (string.IsNullOrWhiteSpace(file)) return new Version();
            return Version.TryParse(FileVersionInfo.GetVersionInfo(file).FileVersion,
                out var version) ? version : new Version();
        }
        catch (FileNotFoundException) { return new Version(); }
    }

    static void TryDelete(string path)
    {
        try { File.Delete(path); }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }
}
