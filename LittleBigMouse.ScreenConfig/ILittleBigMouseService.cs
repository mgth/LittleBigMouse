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
using System.Threading.Tasks;

namespace LittleBigMouse.ScreenConfig
{
    public interface ILittleBigMouseClientService : ILittleBigMouseService
    {
        event Action<string> StateChanged;
    }

    //    public class LittleBigMouseClient : DuplexClientBase<ILittleBigMouseService>, ILittleBigMouseService
    public class LittleBigMouseClient //: DuplexClientBase<ILittleBigMouseService>
    {


        public static Uri Address => new Uri("net.pipe://localhost/littlebigmouse");
        public LittleBigMouseClient(LittleBigMouseClientService service)
        {
            Service = service;
        }

        public ILittleBigMouseService Service { get; }
    }


    public interface ILittleBigMouseService
    {
        void LoadConfig();
        void Quit();
        void Start();
        void Stop();
        void LoadAtStartup(bool state=true);
        void CommandLine(IList<string> args);
        void Running();
        void Update();
    }

}
