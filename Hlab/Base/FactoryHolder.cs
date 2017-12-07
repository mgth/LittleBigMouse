using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Runtime.CompilerServices;

namespace Hlab.Base
{
    public class Factory<T>
        where T : class
    {
        private readonly ConditionalWeakTable<object, T> _wtable
            = new ConditionalWeakTable<object, T>();

        private readonly ConcurrentDictionary<Type, Func<object, T>> _registered = new ConcurrentDictionary<Type, Func<object, T>>();
        private readonly ConcurrentDictionary<Type, Func<object, T>> _cache = new ConcurrentDictionary<Type, Func<object, T>>();


        public void Register(Type type, Func<object, T> factory)
        {
            if(_registered.TryAdd(type, factory))
            {
                foreach (var t in _cache)
                {
                    _cache.TryUpdate(t.Key,t.Value, GetFactory(t.Key));
                }
            }
        }

        public T Get(object target, Action<T> onCreate=null)
        {
            bool created = false;

             var obj = _wtable.GetValue(target, t =>
            {
                created = true;
                return _cache.GetOrAdd(t.GetType(), GetFactory).Invoke(target);
            });

            if (created) onCreate?.Invoke(obj);

            return obj;

        }

        private Func<object, T> GetFactory(Type type)
        {
            KeyValuePair<Type, Func<object, T>>? bestMatch=null;
            foreach (var entry in _registered)
            {
                if (entry.Key.IsAssignableFrom(type))
                {
                    if (!bestMatch.HasValue || bestMatch.Value.Key.IsAssignableFrom(entry.Key))
                    {
                        bestMatch = entry;
                    }
                }
            }
            return bestMatch?.Value;
        }
    }
}
