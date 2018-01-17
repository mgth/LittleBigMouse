/*
  HLab.Notify.4
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Notify.4.

    HLab.Notify.4 is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Notify.4 is distributed in the hope that it will be useful,
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
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Threading;

namespace HLab.Notify
{
    public class NotifierHandler
    {
        private readonly object _target;
        public NotifierHandler(object target)
        {
            _target = target;
        }

        private event PropertyChangedEventHandler PropertyChanged;
        
        public void Add(PropertyChangedEventHandler handler)
        {
            PropertyChanged += handler;
        }

        public bool Remove(PropertyChangedEventHandler handler)
        {
            PropertyChanged -= handler;
            return (PropertyChanged == null);
        }

        private PropertyChangedEventHandler[] GetHandlers()
        {
            return PropertyChanged?.GetInvocationList().OfType<PropertyChangedEventHandler>().ToArray();
        }

        public  void OnPropertyChanged(PropertyChangedEventArgs args)
        {
            var handlers = GetHandlers();
            if (handlers == null) return;

            //Parallel.ForEach(handlers,handler =>
            foreach (var handler in handlers)
            {
                if (handler.Target is DispatcherObject d)
                {
                    d.Dispatcher.BeginInvoke(DispatcherPriority.DataBind,
                        new Action(() => handler(_target, args)));

                    //var uiContext = TaskScheduler.FromCurrentSynchronizationContext();
                    //Task.Factory.StartNew(() => handler(_target, args), CancellationToken.None, TaskCreationOptions.None, uiContext);

                }
                else
                {
                    handler(_target, args);
                }
            }
            //);
        }
    }
}