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
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using HLab.Base;

namespace HLab.Notify
{
    public class NotifierPropertyChangedEventArgs : PropertyChangedEventArgs
    {
        public object OldValue { get; }
        public object NewValue { get; }
        public NotifierPropertyChangedEventArgs(string propertyName, object oldValue, object newValue):base(propertyName)
        {
            OldValue = oldValue;
            NewValue = newValue;
        }
    }

    //public interface Notifier
    //{
    //    void Add(PropertyChangedEventHandler handler);
    //    void Remove(PropertyChangedEventHandler handler);
    //    T Get<T>(Func<T, T> getter, string propertyName, Action postUpdateAction = null);
    //    bool Set<T>(T value, string propertyName, Action<T, T> postUpdateAction = null);
    //    bool Update(string propertyName);
    //    bool IsSet(string propertyName);
    //    void OnPropertyChanged(string propertyName);
    //    Suspender Suspend { get; }
    //}

    public class Notifier /*: Notifier*/
    {
        public Notifier(Type classType)
        {
            Class = NotifierService.D.GetNotifierClass(classType);
        }

        private readonly ConcurrentDictionary<INotifyPropertyChanged,NotifierHandler> _weakTable = new ConcurrentDictionary<INotifyPropertyChanged, NotifierHandler>();
        public void Add(INotifyPropertyChanged obj, PropertyChangedEventHandler handler)
        {
            var h = _weakTable.GetOrAdd(obj,o => new NotifierHandler(o));
            h.Add(handler);
        }

        public void Remove(INotifyPropertyChanged obj, PropertyChangedEventHandler handler)
        {
            var h = _weakTable.GetOrAdd(obj, o => new NotifierHandler(o));
            if (h.Remove(handler)) _weakTable.TryRemove(obj, out h);
        }

        public object Target => _weakTable.Select(k => k.Key).FirstOrDefault();

        public NotifierClass Class { get; }

        private Suspender _suspender;
        public Suspender Suspend => _suspender ?? (_suspender = new Suspender(OnPropertyChanged));

        private readonly ConcurrentQueue<PropertyChangedEventArgs> _queue = new ConcurrentQueue<PropertyChangedEventArgs>();

        private void OnPropertyChanged(PropertyChangedEventArgs args)
        {
            using (Suspend.Get())
                _queue.Enqueue(args);
        }

        private void OnPropertyChanged()
        {
            while (_queue.TryDequeue(out var args))
            {
                foreach(var h in _weakTable.Values)
                    h.OnPropertyChanged(args);
            }
        }





        public void OnPropertyChanged(string propertyName)
        {
            OnPropertyChanged(new PropertyChangedEventArgs(propertyName));
        }


        protected readonly ConcurrentDictionary<NotifierProperty, NotifierEntry> Entries =
            new ConcurrentDictionary<NotifierProperty, NotifierEntry>();


        public T Get<T>(object target, Func<T, T> getter, string propertyName, Action postUpdateAction = null)
        {
            Debug.Assert(!string.IsNullOrWhiteSpace(propertyName), "propertyName cannot be null or empty");
            return Get(target, getter, Class.GetProperty(propertyName), postUpdateAction);
        }

        public T Get<T>(object target, Func<T, T> getter, NotifierProperty property, Action postUpdateAction = null)
        {
            try
            {
                return Entries.GetOrAdd(property,
                    p => p.GetNewEntry(this, a => getter.Invoke((T) (a ?? default(T))))).GetValue<T>();
            }
            catch (PropertyNotReady ex)
            {
                if (Entries.TryRemove(property, out var e))
                {
                    
                };
                return (T)ex.ReturnValue;
            }
            catch (NullReferenceException ex)
            {
                if (Entries.TryRemove(property, out var e))
                {
                    
                }
                return default(T);
            }

        }


        public bool Set<T>(object target, T value, string propertyName, Action<T, T> postUpdateAction = null)
        {
            Debug.Assert(!string.IsNullOrWhiteSpace(propertyName), "propertyName cannot be null or empty");

            return Set(target, value, Class.GetProperty(propertyName), postUpdateAction);
        }

        public bool SetOneToMany<T, TNotifier>(TNotifier target, T value, Func<T, IList<TNotifier>> getCollection, string propertyName)
            where TNotifier : INotifierObject
        {
            Debug.Assert(!string.IsNullOrWhiteSpace(propertyName), "propertyName cannot be null or empty");

            return Set(target, value, Class.GetProperty(propertyName), (oldValue, newValue) =>
            {
                if(oldValue!=null) getCollection(oldValue).Remove(target);
                if(newValue!=null) getCollection(newValue).Add(target);
            });
        }

        public bool Set<T>(object target, T value, NotifierProperty property, Action<T, T> postUpdateAction = null)
        {
            
            var isnew = false;

            var entry = Entries.GetOrAdd(property, (oldValue) =>
                {
                    isnew = true;
                    return property.GetNewEntry(this, n => value);
                }
            );

            var old = isnew?default(T):entry.GetValue<T>();

            if (isnew || entry.SetValue(value))
            {
                postUpdateAction?.Invoke(old, value);
                OnPropertyChanged(new NotifierPropertyChangedEventArgs(property.Name, old, value));
                return true;
            }

            return false;
        }

        public bool IsSet(PropertyInfo property) => IsSet(Class.GetProperty(property));
        public bool IsSet(NotifierProperty property) => Entries.ContainsKey(property);

        public bool Update(PropertyInfo property) => Update(Class.GetProperty(property));
        public bool Update(NotifierProperty property)
        {
            if (Entries.TryGetValue(property, out var entry))
            {
                var old = entry.GetValue<object>();

                if (entry.Update())
                {
                    OnPropertyChanged(new NotifierPropertyChangedEventArgs(property.Name, old, entry.GetValue<object>()));
                    return true;
                }
            }
            return false;
        }



        private bool _subscribed = false;
        public void Subscribe(INotifyPropertyChanged n)
        {
            if(_subscribed) throw new InvalidOperationException("Notifier subscribed twice");
            _subscribed = true;

            foreach (var method in n.GetType().GetMethods())
            {
                foreach (var triggedOn in method.GetCustomAttributes().OfType<TriggedOn>())
                {
                    switch (method.GetParameters().Length)
                    {
                        case 0:
                            n.SubscribeNotifier((s,args) => method.Invoke(n, null), triggedOn.Pathes);
                            break;
                        case 1:
                            n.SubscribeNotifier((s, args) => method.Invoke(n, new object[] { args }), triggedOn.Pathes);
                            break;
                        case 2:
                            n.SubscribeNotifier((s, args) => method.Invoke(n, new object[] { s, args }), triggedOn.Pathes);
                            break;
                    }
                }
            }

            foreach (var property in n.GetType().GetProperties())
            {
                var name = property.Name;
                foreach (var triggedOn in property.GetCustomAttributes().OfType<TriggedOn>())
                {
                    if (typeof(ITriggable).IsAssignableFrom(property.PropertyType))
                    {
                        n.SubscribeNotifier((s,a) =>
                        {
                            if (IsSet(property))
                                Get<ITriggable>(n, null, name).OnTrigged();
                            else
                                //OnPropertyChanged(name);
                                OnPropertyChanged(new NotifierPropertyChangedEventArgs(property.Name, null, null));
                            //property.GetMethod.Invoke(n, null);

                        }, triggedOn.Pathes);
                    }
                    else
                    {
                        n.SubscribeNotifier((s,a) =>
                        {
                            if (IsSet(property))
                                Update(property);
                            else
                                //OnPropertyChanged(name);
                                OnPropertyChanged(new NotifierPropertyChangedEventArgs(property.Name, null, null));
                            //property.GetMethod.Invoke(n, null);

                        }, triggedOn.Pathes);
                    }
                }
            }
        }
    }
}
