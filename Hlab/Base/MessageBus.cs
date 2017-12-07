using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;

namespace Hlab.Base
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
