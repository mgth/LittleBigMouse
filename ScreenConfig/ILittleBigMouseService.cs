using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Text;
using System.Threading.Tasks;

namespace LbmScreenConfig
{
    public class LittleBigMouseClient : ClientBase<ILittleBigMouseService>, ILittleBigMouseService
    {
        private static LittleBigMouseClient _client;

        public static LittleBigMouseClient Client
        {
            get
            {
                if (_client == null)
                {
                    _client = new LittleBigMouseClient();
                }

                if (_client.InnerChannel.State == CommunicationState.Faulted)
                {
                    _client.Abort();
                    _client = new LittleBigMouseClient();
                }

                return _client;
            }
        }

        public static Uri Address => new Uri("net.pipe://localhost/littlebigmouse");
        public LittleBigMouseClient()
        : base(new ServiceEndpoint(ContractDescription.GetContract(typeof(ILittleBigMouseService)),
            new NetNamedPipeBinding(), new EndpointAddress(Address)))
    {

        }

 
        public void LoadConfig()
        {
            try { Channel.LoadConfig(); }
            catch (EndpointNotFoundException) { LauchServer("--loadconfig"); }
            catch (FaultException)
            {
            }
        }

        public void Quit()
        {
            try { Channel.Quit(); }
            catch (EndpointNotFoundException) 
            { }
            catch (FaultException)
            {
            }
        }

        public void Start()
        {
            try { Channel.Start(); }
            catch (EndpointNotFoundException)
            { LauchServer("--start"); }
            catch (FaultException)
            {
            }
        }

        public void Stop()
        {
            try { Channel.Stop(); }
            catch (EndpointNotFoundException) { }
            catch (FaultException)
            {
            }
        }
        public void CommandLine(IList<string> args)
        {
            try { Channel.CommandLine(args); }
            catch (EndpointNotFoundException) { LauchServer(String.Join(".",args)); }
            catch (FaultException)
            {
            }
        }

        public void LoadAtStartup(bool state = true)
        {
            try
            {
                Channel.LoadAtStartup(state);
            }
            catch (EndpointNotFoundException)
            {
                LauchServer(state ? "--schedule" : "--unschedule");
            }
            catch (FaultException)
            {
            }
        }
        public void LauchServer(string args="")
        {
            var p = Process.GetCurrentProcess();
            string filename = p.MainModule.FileName.Replace("Control", "Daemon").Replace(".vshost", "");
            Process.Start(filename,args);
        }
    }


    [ServiceContract]
    public interface ILittleBigMouseService
    {
        [OperationContract]
        void LoadConfig();
        [OperationContract]
        void Quit();
        [OperationContract]
        void Start();
        [OperationContract]
        void Stop();
        [OperationContract]
        void LoadAtStartup(bool state=true);
        [OperationContract]
        void CommandLine(IList<string> args);
    }
}
