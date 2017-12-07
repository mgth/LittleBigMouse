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
using System.ComponentModel;
using System.Diagnostics;
using Hlab.Base;

namespace Hlab.Notify
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


        protected readonly ConcurrentDictionary<string, NotifierEntry> Entries =
            new ConcurrentDictionary<string, NotifierEntry>();

        public T Get<T>(Func<T, T> getter, string propertyName, Action postUpdateAction = null)
        {
            Debug.Assert(!string.IsNullOrWhiteSpace(propertyName), "propertyName cannot be null or empty");
            try
            {
                return Entries.GetOrAdd(propertyName,
                    e => new NotifierEntry(this, a => getter.Invoke((T) (a ?? default(T))))).GetValue<T>();
            }
            catch (PropertyNotReady ex)
            {
                if (Entries.TryRemove(propertyName, out var e))
                {
                    
                };
                return (T)ex.ReturnValue;
            }
            catch (NullReferenceException ex)
            {
                if (Entries.TryRemove(propertyName, out var e))
                {
                    
                }
                return default(T);
            }

        }
        public bool Set<T>(T value, string propertyName, Action<T, T> postUpdateAction = null)
        {
            Debug.Assert(!string.IsNullOrWhiteSpace(propertyName), "propertyName cannot be null or empty");
            
            bool isnew = false;

            var entry = (NotifierEntry) Entries.GetOrAdd(propertyName, (oldValue) =>
                {
                    isnew = true;
                    return new NotifierEntry(this, n => value);
                }
            );

            if (isnew)
            {
                OnPropertyChanged(new NotifierPropertyChangedEventArgs(propertyName, default(T), value));
                return true;
            }

            var old = entry.GetValue<T>();

            if (entry.SetValue(value))
            {
                postUpdateAction?.Invoke(old, value);
                OnPropertyChanged(new NotifierPropertyChangedEventArgs(propertyName, old, value));
                return true;
            }

            return false;
        }

        public bool IsSet(string propertyName) => Entries.ContainsKey(propertyName);

        public bool Update(string propertyName)
        {
            if (Entries.TryGetValue(propertyName, out var entry))
            {
                var old = entry.GetValue<object>();

                if (entry.Update())
                {
                    OnPropertyChanged(new NotifierPropertyChangedEventArgs(propertyName, old, entry.GetValue<object>()));
                    return true;
                }
            }
            return false;
        }
    }
}
