/*
  LittleBigMouse.Screen.Config
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Screen.Config.

    LittleBigMouse.Screen.Config is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Screen.Config is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.ServiceModel;
using System.ServiceModel.Description;

namespace LittleBigMouse.ScreenConfigs
{
    //    public class LittleBigMouseClient : DuplexClientBase<ILittleBigMouseService>, ILittleBigMouseService
    public class LittleBigMouseClient : ClientBase<ILittleBigMouseService>, ILittleBigMouseService
    {
        private static LittleBigMouseClient _client;

        //private static readonly ILittleBigMouseCallback _callback = new MyCallbackClient();

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
        : base(new ServiceEndpoint( ContractDescription.GetContract(typeof(ILittleBigMouseService)),
            new NetNamedPipeBinding(), new EndpointAddress(Address)))
    {
            //Init();
        }


        //public void Init()
        //{
        //    try { Channel.Init(); }
        //    catch (EndpointNotFoundException) { }
        //    catch (CommunicationException) { }
        //}

        public void LoadConfig()
        {
            try { Channel.LoadConfig(); }
            catch (EndpointNotFoundException) { LauchServer("--loadconfig"); }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }

        public void Quit()
        {
            try { Channel.Quit(); }
            catch (EndpointNotFoundException)  { }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }

        public void Start()
        {
            try { Channel.Start(); }
            catch (EndpointNotFoundException) { LauchServer("--start"); }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }

        public void Stop()
        {
            try { Channel.Stop(); }
            catch (EndpointNotFoundException) { }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }
        public void CommandLine(IList<string> args)
        {
            try { Channel.CommandLine(args); }
            catch (EndpointNotFoundException) { LauchServer(String.Join(".",args)); }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }

        public bool Running()
        {
            try
            {
                return Channel.Running();
            }
            catch (EndpointNotFoundException) {}
            catch (CommunicationException) {}

            return false;
        }

        public void LoadAtStartup(bool state = true)
        {
            try
            {
                Channel.LoadAtStartup(state);
            }
            catch (EndpointNotFoundException) { LauchServer(state ? "--schedule" : "--unschedule"); }
            catch (CommunicationException) { }
            //catch (FaultException) { }
        }

        public void LauchServer(string args="")
        {
            var p = Process.GetCurrentProcess();
            string filename = p.MainModule.FileName.Replace("_Control", "_Daemon").Replace(".vshost", "");
            Process.Start(filename,args);
            //Thread.Sleep(1000);
            //Init();
        }

        public event Action StateChanged; 

        public void OnStateChange()
        {
            StateChanged?.Invoke();
        }
    }


//    [ServiceContract(CallbackContract = typeof(ILittleBigMouseCallback))]
    [ServiceContract]
    public interface ILittleBigMouseService
    {
        //[OperationContract]
        //void Init();
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
        [OperationContract]
        bool Running();
    }

    [ServiceContract]
    public interface ILittleBigMouseCallback
    {
        [OperationContract]
        void OnStateChange();
    }

    public class MyCallbackClient : ILittleBigMouseCallback
    {
        public void OnStateChange()
        {
            LittleBigMouseClient.Client.OnStateChange();
        }
    }
}
