using System;
using LbmScreenConfig;
using System.ServiceModel;
using System.ServiceProcess;
using System.Threading;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    class Program
    {
        private const string Unique = "LittleBigMouse_Daemon";
        [STAThread]
        public static void Main(string[] args)
        {
            bool firstInstance;
            Mutex mutex = new Mutex(true, Unique + Environment.UserName, out firstInstance);

            if (!firstInstance)
            {
                LittleBigMouseClient.Client.CommandLine(args);
                mutex.Close();
                return;
            }

            if (Environment.UserInteractive)
            {
                LittleBigMouseDaemon daemon = new LittleBigMouseDaemon();
                daemon.Run();
            }
            else
            {
                var servicesToRun = new ServiceBase[]
                {
                    new LittleBigMouseService()
                };
                ServiceBase.Run(servicesToRun);
            }

            mutex.Close();
        }
    }
}
