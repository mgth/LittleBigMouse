/*
  HLab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Base.

    HLab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Base is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;

namespace HLab.Base
{
    public sealed class MessageBus : Singleton<MessageBus>
    {
        private MessageBus() { }

        private readonly object _lockDict = new object();
        private readonly Dictionary<Type, IList> _dict = new Dictionary<Type, IList>();

        private readonly object _lockPayload = new object();
        private readonly Dictionary<Type, object> _payload = new Dictionary<Type, object>();

        public void Publish<T>(T payload)
        {
            _payload[typeof(T)] = payload;

            List<IList> dict;
            lock (_lockDict)
            {
               dict = _dict.Where(t => t.Key.IsAssignableFrom(typeof(T))).Select(e => e.Value).ToList();
            }

            foreach (var list in dict)
            {
                if (list == null) continue;
                foreach (var action in list.OfType<Action<T>>().ToList())
                {
                    action(payload);
                }
            }
        }

        public void Subscribe<T>(Action<T> action)
        {
            lock (_lockDict)
            {
                List<Action<T>> list = null;

                if (_dict.ContainsKey(typeof(T)))
                    list = _dict[typeof(T)] as List<Action<T>>;

                if (list == null)
                {
                    list = new List<Action<T>>();
                    _dict.Add(typeof(T), list);
                }
                list.Add(action);
            }

            if (_payload.ContainsKey(typeof(T))) action((T)_payload[typeof(T)]);
        }

        public void Unsubscribe<T>(Action<T> action)
        {
            lock (_lockDict)
            {
                var list = _dict[typeof(T)] as List<Action<T>>;
                if (list != null)
                {
                    list.Remove(action);
                    if (list.Count == 0) _dict.Remove(typeof(T));
                }
            }
        }
    }

}
