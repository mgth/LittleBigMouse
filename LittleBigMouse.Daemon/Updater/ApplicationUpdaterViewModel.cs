using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Input;
using HLab.Mvvm;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using LittleBigMouse_Daemon.Updater;
using Newtonsoft.Json.Linq;
using Application = System.Windows.Application;

namespace LittleBigMouse.Daemon.Updater
{
    using H = H<ApplicationUpdateViewModel>;

    public class ApplicationUpdateViewModel : ViewModel
    {
        public ApplicationUpdateViewModel()
        {
            H.Initialize(this);
        }


        public void Show()
        {
            var view = new ApplicationUpdateView
            {
                DataContext = this
            };
            view.ShowDialog();
        }

        public double Progress
        {
            get => _progress.Get();
            set => _progress.Set(value);
        }
        private readonly IProperty<double> _progress = H.Property<double>();

        public bool Updated
        {
            get => _updated.Get();
            set => _updated.Set(value);
        }
        private readonly IProperty<bool> _updated = H.Property<bool>();

        public Version CurrentVersion => _currentVersion.Get();
        private readonly IProperty<Version> _currentVersion = H.Property<Version>();

        [TriggerOn]
        private void _setVersion()
        {
            var v = FileVersionInfo.GetVersionInfo("LittleBigMouse_Control.exe").FileVersion;
            if (Version.TryParse(v, out var version)) _currentVersion.Set(version) ;
            _currentVersion.Set(null);
        }

        public Version NewVersion
        {
            get => _newVersion.Get();
            set => _newVersion.Set(value);
        }
        private readonly IProperty<Version> _newVersion = H.Property<Version>();

        // http://www.chmp.org/sites/default/files/apps/sampling/
        public string Url
        {
            get => _url.Get();
            set => _url.Set(value);
        }
        private readonly IProperty<string> _url = H.Property<string>();

        public string FileName
        {
            get => _fileName.Get();
            set => _fileName.Set(value);
        }
        private readonly IProperty<string> _fileName = H.Property<string>();

        private readonly IProperty<string> _message = H.Property<string>();
        public string Message
        {
            get => _message.Get();
            set => _message.Set(value);
        }

        public ICommand UpdateCommand { get; }  = H.Command(
            c=>c
                .CanExecute(e => e.NewVersionFound)
                .Action(e => e.Update())
                .On(e => e.NewVersionFound).CheckCanExecute()
            );


        [TriggerOn(nameof(NewVersion))]
#if DEBUG
        public bool NewVersionFound => true;
#else
        public bool NewVersionFound => NewVersion > CurrentVersion;
#endif
        private void Update()
        {
            var filename = Url.Split('/').Last(); //FileName.Replace("{version}", NewVersion.ToString());
            var path = Path.GetTempPath() + filename;

            var thread = new Thread(() =>
            {
                var client = new WebClient();
                client.DownloadProgressChanged += client_DownloadProgressChanged;
                client.DownloadFileCompleted += client_DownloadFileCompleted;
                client.DownloadFileAsync(new Uri(Url), path);
            });
            thread.Start();
        }

        private void client_DownloadProgressChanged(object sender, DownloadProgressChangedEventArgs e)
        {
            var bytesIn = double.Parse(e.BytesReceived.ToString());
            var totalBytes = double.Parse(e.TotalBytesToReceive.ToString());
            Progress = bytesIn / totalBytes * 100;
        }

        private void client_DownloadFileCompleted(object sender, AsyncCompletedEventArgs e)
        {
            RunUpdate();
        }

        private void RunUpdate()
        {
            var filename = Url.Split('/').Last(); //FileName.Replace("{version}", NewVersion.ToString());
            var path = Path.GetTempPath() + filename;
            var startInfo = new ProcessStartInfo(path) {Verb = "runas"};
            try
            {
                Process.Start(startInfo);
                Updated = true;
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

        private void Update2()
        {
            var filename = FileName.Replace("{version}", NewVersion.ToString());
            var path = Path.GetTempPath() + filename;

            var request = WebRequest.CreateHttp(Url + filename);

            request.Method = "GET";


            try
            {
                var response = (HttpWebResponse) request.GetResponse();

                var streamResponse = response.GetResponseStream();
                if (streamResponse == null) return;

                using (var fileStream =
                    new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.None))
                {
                    streamResponse.CopyTo(fileStream);
                }

                var startInfo = new ProcessStartInfo(path) {Verb = "runas"};
                Process.Start(startInfo);
                Application.Current.Shutdown();
            }
            catch (Win32Exception)
            {
                Message = "L'execution a échouée";
            }
            catch (WebException)
            {
                Message = "Le téléchargement a échoué";
            }
        }

        public async Task CheckVersion()
        {
            using (var client = new HttpClient())
            {
                client.DefaultRequestHeaders.Add("User-Agent", "LittleBigMouse");
                var r = await client.GetAsync("https://api.github.com/repos/Mgth/LittleBigMouse/releases");
                var json = JArray.Parse(r.Content.ReadAsStringAsync().Result);

                var name = json[0]["name"].Value<string>();

                Message = json[0]["body"].ToString();

                Url = json[0]["assets"][0]["browser_download_url"].ToString();

                if (Version.TryParse(name, out var onlineVersion))
                {
                    NewVersion = onlineVersion;
                }
            }
        }
    }
}
