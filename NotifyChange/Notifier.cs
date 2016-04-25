using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Windows.Threading;

namespace NotifyChange
{
    public class NotifierSuspend : IDisposable
    {
        private readonly Notifier _notifier;

        public NotifierSuspend(Notifier notifier)
        {
            _notifier = notifier;
        }

        public void Dispose()
        {
            _notifier.Resume();
        }
    }

    public class Notifier : INotifyPropertyChanged
    {
        public Notifier()
        {
            var suspend = Suspend();
            // Notification will start after full object initialisation
            Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.Send,
                new Action(() => { suspend.Dispose(); }));
        }

        protected virtual INotifyPropertyChanged Notify => this;

        public event PropertyChangedEventHandler PropertyChanged;

        private int _suspended = 0;
        private readonly object _lockSuspended = new object();


        private readonly HashSet<string> _propertyChangedList = new HashSet<string>();

        public NotifierSuspend Suspend()
        {
            var s = new NotifierSuspend(this);
            lock (_lockSuspended)
            {
                _suspended++;               
            }
            return s;
        }

        public void Resume()
        {
            lock (_lockSuspended)
            {
                _suspended--;
            }
            OnPropertyChanged();
        }

        protected void OnPropertyChanged(string propName)
        {
            lock (_lockSuspended)
            {
                if (!_propertyChangedList.Contains(propName)) _propertyChangedList.Add(propName);
            }
            OnPropertyChanged();
        }


        protected void OnPropertyChanged()
        {
            List<string> tmp;
            Delegate[] delegates;

            lock (_lockSuspended)
            {
                if (_suspended > 0) return;
                tmp = _propertyChangedList.Where(s => !string.IsNullOrEmpty(s)).ToList();
                if (tmp.Count == 0) return;
                _propertyChangedList.Clear();
                if (PropertyChanged == null) return;
                delegates = PropertyChanged.GetInvocationList().ToArray();
            }



            foreach (var s in tmp)
            {
                var arg = new PropertyChangedEventArgs(s);

                foreach (PropertyChangedEventHandler sink in delegates.Cast<PropertyChangedEventHandler>())
                {
                    sink(Notify, arg);
                }
            }
        }


        public bool SetProperty<T>(ref T storage, T value,
            [CallerMemberName] string propertyName = null)
        {
            if (Equals(storage, value)) return false;
            var old = storage;
            storage = value;
            RaiseProperty(propertyName, old, value);
            return true;
        }
        public bool SetAndWatch<T>(ref T storage, T value, [CallerMemberName] string propertyName = null) where T : INotifyPropertyChanged
        {
            if (storage != null && !Equals(storage, value)) UnWatch(storage, propertyName);
            if (SetProperty(ref storage, value, propertyName))
            {
                Watch(storage, propertyName);
                return true;
            }
            return false;
        }

        private readonly Dictionary<string, object> _values = new Dictionary<string, object>(); 
        public bool SetProperty<T>(T value, [CallerMemberName] string propertyName = null)
        {
            T old;

            if (_values.ContainsKey(propertyName))
            {
                old = GetProperty<T>(propertyName);
                if (Equals(old, value)) return false;
                _values[propertyName] = value;
            }
            else
            {
                old = default(T);
                _values.Add(propertyName,value);
            }

            RaiseProperty(propertyName, old, value);
            return true;
        }

        public T GetProperty<T>([CallerMemberName] string propertyName = null)
        {
            if (propertyName!=null && _values.ContainsKey(propertyName))
                return (T)_values[propertyName];

            RaiseProperty("init_" + propertyName);

            if (propertyName != null && _values.ContainsKey(propertyName))
                return (T)_values[propertyName];

            return default(T);
        }
        //public bool SetAndWatch<T>(T value, [CallerMemberName] string propertyName = null) where T : INotifyPropertyChanged
        //{
        //    T old = GetProperty<T>(propertyName);
        //    if (old != null && !Equals(old, value)) UnWatch(old, propertyName);
        //    if (SetProperty(value, propertyName))
        //    {
        //        Watch(propertyName);
        //        return true;
        //    }
        //    return false;
        //}
        public bool SetAndWatch<T>(T value, [CallerMemberName] string propertyName = null) where T : INotifyPropertyChanged
        {
            T old;

            if (_values.ContainsKey(propertyName))
            {
                old = GetProperty<T>(propertyName);
                if (Equals(old, value)) return false;
                UnWatch(old, propertyName);
                _values[propertyName] = value;
            }
            else
            {
                old = default(T);
                _values.Add(propertyName, value);
            }

            Watch(propertyName);

            RaiseProperty(propertyName, old, value);
            return true;
        }


        private bool _init = false;
        public void InitNotifier()
        {
            //Type t = Notify.GetType();

            //while (t != null)
            //{
            //    MethodInfo[] m = t.GetMethods(
            //        BindingFlags.DeclaredOnly |
            //        BindingFlags.Instance | BindingFlags.Public
            //        | BindingFlags.NonPublic
            //        );
            //    foreach (MethodInfo mi in m)
            //    {
            //        if (mi.GetCustomAttributes(false).OfType<DependsOn>().Any())
            //        {
            //            if (mi.GetParameters().Length == 0)
            //            {
            //                mi.Invoke(Notify, null);

            //            }
            //            else
            //            {
            //                mi.Invoke(Notify, new object[] { "_init_" });
            //            }
            //        }
            //    }

            //    t = t.BaseType;
            //}
            RaiseProperty("_init_");
            _init = true;
        }

        private void GetDependOn(string propertyName, HashSet<string> list, HashSet<MethodInfo> listMethods)
        {
            list.Add(propertyName);

            Type t = Notify.GetType();
            while (t != null)
            {
                PropertyInfo[] props = t.GetProperties(
                    BindingFlags.NonPublic
                    |  BindingFlags.Public
                    | BindingFlags.Instance 
               //     | BindingFlags.Static             
                    );
                foreach (PropertyInfo pInfo in props)
                {
                    foreach (DependsOn ca in pInfo.GetCustomAttributes(false).OfType<DependsOn>())
                    {
                        if (ca.Properties != null)
                        {
                            if (
                                ca.Properties.Any(s => s == propertyName // Exact match
                                    || propertyName.Like(s)
                                    || s.Split('.')[0] == propertyName) 
                                
                                && !list.Contains(pInfo.Name))
                            {
                                GetDependOn(pInfo.Name, list, listMethods);
                            }
                        }
                    }
                }

                MethodInfo[] m = t.GetMethods(
                    BindingFlags.DeclaredOnly |
                    BindingFlags.Instance | BindingFlags.Public
                    | BindingFlags.NonPublic
                    );
                foreach (MethodInfo mi in m)
                {
                    if (!listMethods.Contains(mi))
                    {
                        foreach (DependsOn ca in mi.GetCustomAttributes(false).OfType<DependsOn>())
                        {
                            if (ca.Properties == null) continue;
                            //if ( ca.Properties.Contains(propertyName) || ca.Properties.Contains(propertyName) )

                            if (
                                ca.Properties.Any(s => s == propertyName // Exact match
                                || propertyName.Like(s)
                                || s.Split('.')[0] == propertyName))
                            {
                                listMethods.Add(mi);
                            }
                        }
                    }
                }

                t = t.BaseType;
            }
        }

        private static readonly Dictionary<Type, Dictionary<string, HashSet<string>>> _dictProperties = new Dictionary<Type, Dictionary<string, HashSet<string>>>();
        private static readonly Dictionary<Type, Dictionary<string, HashSet<MethodInfo>>> _dictMethods = new Dictionary<Type, Dictionary<string, HashSet<MethodInfo>>>();

        private readonly Dictionary<Tuple<INotifyPropertyChanged,string>,PropertyChangedEventHandler> _watch = new Dictionary<Tuple<INotifyPropertyChanged, string>, PropertyChangedEventHandler>();
        private readonly Dictionary<INotifyCollectionChanged, NotifyCollectionChangedEventHandler> _watchCollection = new Dictionary<INotifyCollectionChanged, NotifyCollectionChangedEventHandler>();

        private static readonly object LockDict = new object();

        public void RaiseProperty([CallerMemberName] string propertyName = null, object oldValue = null, object newValue = null)
        {
            Dictionary<string, HashSet<string>> dictClassProperties = null;
            Dictionary<string, HashSet<MethodInfo>> dictClassMethods = null;
            HashSet<string> listProperties;
            HashSet<MethodInfo> listMethods;

            bool properties = !(propertyName?.StartsWith("init_")??false);

            Type type = Notify.GetType();
            lock (LockDict)
            {
                
                if (!_dictProperties.TryGetValue(type, out dictClassProperties))
                {
                    dictClassProperties = new Dictionary<string, HashSet<string>>();
                    _dictProperties.Add(type, dictClassProperties);
                }

                if (!_dictMethods.TryGetValue(type, out dictClassMethods))
                {
                    dictClassMethods = new Dictionary<string, HashSet<MethodInfo>>();
                    _dictMethods.Add(type, dictClassMethods);
                }

                if (!dictClassProperties.ContainsKey(propertyName))
                {
                    listProperties = new HashSet<string>();
                    listMethods = new HashSet<MethodInfo>();

                    GetDependOn(propertyName, listProperties, listMethods);
                    dictClassProperties.Add(propertyName, listProperties);
                    dictClassMethods.Add(propertyName, listMethods);
                }
                else
                {
                    listProperties = dictClassProperties[propertyName];
                    listMethods = dictClassMethods[propertyName];
                }
            }

            //Raise property changed event for every properties marked with DependsOn
            if(properties)
            foreach (var s in listProperties)//.Where(s => !s.Contains('.')))
                OnPropertyChanged(s);

            //Execute all methods marked with DependsOn
            if (listMethods != null)
                foreach (var mi in listMethods)
                {
                    var param = mi.GetParameters();

                    switch (param.Length)
                    {
                        case 0:
                            Action action = (Action)Delegate.CreateDelegate(typeof(Action), Convert.ChangeType(Notify, type), mi);
                            action();
                            break;
                        case 1:
                            if (param[0].ParameterType == typeof (string))
                            {
                                Action<string> actionString = (Action<string>)Delegate.CreateDelegate(typeof(Action<string>), Convert.ChangeType(Notify, type), mi);
                                actionString(propertyName);
                                //mi.Invoke(Convert.ChangeType(Notify, type), new object[] { propertyName });
                            }
                            else //if (param[0].ParameterType == type)
                                mi.Invoke(Convert.ChangeType(Notify, type), new[] {newValue});
                            break;
                        case 2:
                            mi.Invoke(Convert.ChangeType(Notify, type), new object[] { oldValue, newValue });
                            break;

                    }
                }
        }


        /// <summary>
        /// Start receiving messages from a specific obj, adding 'prefix.'
        /// </summary>
        /// <param name="obj"></param>
        /// <param name="prefix"></param>
        public void Watch(INotifyPropertyChanged obj, string prefix)
        {
            if (obj != null)
            {
                PropertyChangedEventHandler handler = delegate (object sender, PropertyChangedEventArgs args)
                {
                    // Todo : hack to prevent reintrance 
                    if (args.PropertyName.Split('.').Contains(prefix)) return;
                    RaiseProperty(prefix + "." + args.PropertyName);
                };
                obj.PropertyChanged += handler;

                _watch.Add( Tuple.Create(obj,prefix), handler);
            }
        }
        public void Watch(string prefix)
        {
            INotifyPropertyChanged obj = GetProperty<INotifyPropertyChanged>(prefix);
            if (obj != null)
            {
                PropertyChangedEventHandler handler = delegate (object sender, PropertyChangedEventArgs args)
                {
                    RaiseProperty(prefix + "." + args.PropertyName);
                };
                obj.PropertyChanged += handler;

                _watch.Add(Tuple.Create(obj, prefix), handler);
            }
        }

        /// <summary>
        /// Stop wayching for that obj
        /// </summary>
        /// <param name="obj"></param>
        public void UnWatch(INotifyPropertyChanged obj, string prefix)
        {
            if (obj != null)
            {
                var key = Tuple.Create(obj, prefix);

                if (_watch.ContainsKey(key))
                {
                    PropertyChangedEventHandler handler = _watch[key];
                    obj.PropertyChanged -= handler;
                    _watch.Remove(key);
                }
            }
        }
        public void UnWatch( string prefix)
        {
            INotifyPropertyChanged obj = GetProperty<INotifyPropertyChanged>(prefix);
            UnWatch(obj,prefix);
        }

        /// <summary>
        /// Receive message foreach item of collection
        /// </summary>
        /// <typeparam name="T"></typeparam>
        /// <param name="collection"></param>
        /// <param name="prefix"></param>
        public void Watch<T>(ObservableCollection<T> collection, string prefix) where T : INotifyPropertyChanged
        {
            if (collection == null) return;

            if (_watchCollection.ContainsKey(collection)) { UnWatch(collection,prefix); }

            foreach (T obj in collection) Watch(obj, prefix);

            NotifyCollectionChangedEventHandler handler = delegate (object sender, NotifyCollectionChangedEventArgs e)
            {
                if (e.OldItems != null)
                    foreach (INotifyPropertyChanged item in e.OldItems)
                    {
                        UnWatch(item,prefix);
                        RaiseProperty(prefix);
                    }

                if (e.NewItems != null)
                    foreach (INotifyPropertyChanged item in e.NewItems)
                    {
                        Watch(item, prefix);
                        RaiseProperty(prefix);
                    }
            };

            collection.CollectionChanged += handler;
            _watchCollection.Add(collection, handler);
        }

        public void UnWatch<T>(ObservableCollection<T> collection, string prefix) where T : INotifyPropertyChanged
        {
            if (collection == null) return;

            foreach (T obj in collection) UnWatch(obj,prefix);

            if (_watchCollection.ContainsKey(collection))
            {
                NotifyCollectionChangedEventHandler handler = _watchCollection[collection];
                collection.CollectionChanged -= handler;
                _watchCollection.Remove(collection);
            }
        }
    }
}
