using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
using HLab.Mvvm.Commands;
using HLab.Notify;
using Newtonsoft.Json.Linq;
using Application = System.Windows.Application;

namespace LittleBigMouse_Daemon.Updater
{
    public class ApplicationUpdateViewModel : INotifyPropertyChanged
    {
        public ApplicationUpdateViewModel()
        {
            this.SubscribeNotifier();
        }

        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
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
            get => this.Get<double>();
            set => this.Set(value);
        }

        public bool Updated
        {
            get => this.Get<bool>(() => false);
            set => this.Set(value);
        }

        public Version CurrentVersion => this.Get(() =>
        {
            var v = FileVersionInfo.GetVersionInfo("LittleBigMouse_Control.exe").FileVersion;
            if (Version.TryParse(v, out var version)) return version;
            return null;
        });

        public Version NewVersion
        {
            get => this.Get<Version>();
            set => this.Set(value);
        }

        // http://www.chmp.org/sites/default/files/apps/sampling/
        public String Url
        {
            get => this.Get<string>();
            set => this.Set(value);
        }

        public String FileName
        {
            get => this.Get<string>();
            set => this.Set(value);
        }

        public String Message
        {
            get => this.Get<string>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(NewVersionFound))]
        public ModelCommand UpdateCommand => this.GetCommand(Update, () => NewVersionFound);

        [TriggedOn(nameof(NewVersion))]
#if DEBUG
        public bool NewVersionFound => this.Get(() => true);
#else
        public bool NewVersionFound => this.Get(() => NewVersion > CurrentVersion);
#endif
        private void Update()
        {
            var filename = Url.Split('/').Last(); //FileName.Replace("{version}", NewVersion.ToString());
            var path = Path.GetTempPath() + filename;

            Thread thread = new Thread(() =>
            {
                WebClient client = new WebClient();
                client.DownloadProgressChanged +=
                    new DownloadProgressChangedEventHandler(client_DownloadProgressChanged);
                client.DownloadFileCompleted += new AsyncCompletedEventHandler(client_DownloadFileCompleted);
                client.DownloadFileAsync(new Uri(Url), path);
            });
            thread.Start();
        }

        void client_DownloadProgressChanged(object sender, DownloadProgressChangedEventArgs e)
        {
            double bytesIn = double.Parse(e.BytesReceived.ToString());
            double totalBytes = double.Parse(e.TotalBytesToReceive.ToString());
            Progress = bytesIn / totalBytes * 100;
        }

        void client_DownloadFileCompleted(object sender, AsyncCompletedEventArgs e)
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

                Url = json[0]["assets"][0]["browser_download_url"].ToString();

                if (Version.TryParse(name, out var onlineVersion))
                {
                    NewVersion = onlineVersion;
                }
            }
        }
    }
}
