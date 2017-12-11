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
using System.Collections.Concurrent;
using Hlab.Base;

namespace Hlab.Notify
{
    public class NotifierService : Singleton<NotifierService>
    {
        protected NotifierService()
        {
            Factory.Register(typeof(object),o=>new Notifier(o.GetType()));
        }

        public Factory<Notifier> Factory { get; } = new Factory<Notifier>();

        public ConcurrentDictionary<Type,NotifierClass> Classes = new ConcurrentDictionary<Type, NotifierClass>();

        public Notifier GetNotifier( /*INotifyPropertyChanged*/ object target) => Factory.Get(target);

        public NotifierClass GetNotifierClass(Type type) => Classes.GetOrAdd(type, t => new NotifierClass(t));
    }
}
