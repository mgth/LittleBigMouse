/*
  HLab.Mvvm
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Mvvm.

    HLab.Mvvm is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Mvvm is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using HLab.Notify;

namespace HLab.Mvvm.Commands
{
    public static  class CommandServiceExt
    {
        public static ModelCommand GetCommand(
            this INotifyPropertyChanged notifier, 
            Action execute, 
            Func<bool> enabled,
             [CallerMemberName] string propertyName = null)
        {
            return notifier.Get(()=>
            {
                var ret = new ModelCommand(execute,enabled);
                return ret;
            },propertyName);
        }

        public static ModelCommand GetCommand(
            this INotifyPropertyChanged notifier, 
            Action<object> execute,
            Func<bool> enabled,
            [CallerMemberName] string propertyName = null)
        {
            return notifier.Get(() =>
            {
                var ret = new ModelCommand(execute,enabled);
                return ret;
            }, propertyName);
        }

    }
}
