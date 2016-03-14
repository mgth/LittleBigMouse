using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;

namespace NotifyChange
{
    public class Notifier : INotifyPropertyChanged
    {
        protected virtual INotifyPropertyChanged Notify => this;

        public event PropertyChangedEventHandler PropertyChanged;

        private bool _suspended = false;
        private readonly List<string> _propertyChangedList = new List<string>();
        public bool Suspend()
        {
            bool old = _suspended;
            _suspended = true;
            return old;
        }

        public void Resume(bool value)
        {
            _suspended = value;
            if (_suspended) return;

            List<string> tmp = new List<string>(_propertyChangedList.ToArray());
            _propertyChangedList.Clear();

            if (PropertyChanged == null) return;

            Delegate[] delegates = PropertyChanged.GetInvocationList();

            foreach (string s in tmp)
            {
                PropertyChangedEventArgs arg = new PropertyChangedEventArgs(s);

                foreach (Delegate del in delegates)
                {
                    PropertyChangedEventHandler sink = (PropertyChangedEventHandler)del;
                    try
                    {
                        sink(Notify, arg);
                    }
                    catch (Exception)
                    {
                        // ignored
                    }
                }
            }
        }
        protected void OnPropertyChanged([CallerMemberName] string propName = null)
        {
            if (!_propertyChangedList.Contains(propName)) _propertyChangedList.Add(propName);
            Resume(_suspended);
        }
        public bool SetProperty<T>(ref T storage, T value,
            [CallerMemberName] string propertyName = null)
        {
            if (Equals(storage, value)) return false;
            storage = value;
            RaiseProperty(propertyName);
            return true;
        }

        public bool SetAndWatch<T>(ref T storage, T value, [CallerMemberName] string propertyName = null) where T : INotifyPropertyChanged
        {
            if (storage != null && !Equals(storage, value)) UnWatch(storage);
            if (SetProperty(ref storage, value, propertyName))
            {
                Watch(storage, propertyName);
                return true;
            }
            return false;
        }

        public void InitNotifier()
        {
            Type t = Notify.GetType();

            while (t != null)
            {
                MethodInfo[] m = t.GetMethods(
                    BindingFlags.DeclaredOnly |
                    BindingFlags.Instance | BindingFlags.Public
                    | BindingFlags.NonPublic
                    );
                foreach (MethodInfo mi in m)
                {
                    if (mi.GetCustomAttributes(false).OfType<DependsOn>().Any())
                    {
                        if (mi.GetParameters().Length == 0)
                        {
                            mi.Invoke(Notify, null);

                        }
                        else
                        {
                            mi.Invoke(Notify, new object[] { "" });
                        }
                    }
                }

                t = t.BaseType;
            }
        }

        private void GetDependOn(string propertyName, ref List<string> list, ref List<MethodInfo> listMethods)
        {
            list.Add(propertyName);

            Type t = Notify.GetType();
            while (t != null)
            {
                PropertyInfo[] props = t.GetProperties();
                foreach (PropertyInfo pInfo in props)
                {
                    foreach (DependsOn ca in
                        pInfo.GetCustomAttributes(false).OfType<DependsOn>())
                    {
                        if (ca.Properties != null)
                        {
                            if ((ca.Properties.Contains(propertyName.Split('.')[0]) || ca.Properties.Contains(propertyName)) && !list.Contains(pInfo.Name))
                            {
                                GetDependOn(pInfo.Name, ref list, ref listMethods);
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
                        foreach (DependsOn ca in
                            mi.GetCustomAttributes(false).OfType<DependsOn>())
                        {
                            if (ca.Properties == null) continue;
                            //if ( ca.Properties.Contains(propertyName) || ca.Properties.Contains(propertyName) )
                            if (ca.Properties.Any(s => s == propertyName || s.Split('.')[0] == propertyName))
                            {
                                listMethods.Add(mi);
                            }
                        }
                    }
                }

                t = t.BaseType;
            }
        }

        private static readonly Dictionary<Type, Dictionary<string, List<string>>> _dictProperties = new Dictionary<Type, Dictionary<string, List<string>>>();
        private static readonly Dictionary<Type, Dictionary<string, List<MethodInfo>>> _dictMethods = new Dictionary<Type, Dictionary<string, List<MethodInfo>>>();

        private readonly Dictionary<INotifyPropertyChanged, PropertyChangedEventHandler> _watch = new Dictionary<INotifyPropertyChanged, PropertyChangedEventHandler>();
        private readonly Dictionary<INotifyCollectionChanged, NotifyCollectionChangedEventHandler> _watchCollection = new Dictionary<INotifyCollectionChanged, NotifyCollectionChangedEventHandler>();

        private static readonly object LockDict = new object();

        public void RaiseProperty([CallerMemberName] string propertyName = null)
        {
            Dictionary<string, List<string>> dictClassProperties;
            Dictionary<string, List<MethodInfo>> dictClassMethods;
            List<string> listProperties;
            List<MethodInfo> listMethods;

            Type type = Notify.GetType();
            lock (LockDict)
            {

                if (!_dictProperties.TryGetValue(type, out dictClassProperties))
                {
                    dictClassProperties = new Dictionary<string, List<string>>();
                    _dictProperties.Add(type, dictClassProperties);
                }

                if (!_dictMethods.TryGetValue(type, out dictClassMethods))
                {
                    dictClassMethods = new Dictionary<string, List<MethodInfo>>();
                    _dictMethods.Add(type, dictClassMethods);
                }

                if (!dictClassProperties.ContainsKey(propertyName))
                {
                    listProperties = new List<string>();
                    listMethods = new List<MethodInfo>();

                    GetDependOn(propertyName, ref listProperties, ref listMethods);
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
            foreach (var s in listProperties.Where(s => !s.Contains('.')))
                OnPropertyChanged(s);

            //Execute all methods marked with DependsOn
            if (listMethods != null)
                foreach (var mi in listMethods)
                {
                    if (mi.GetParameters().Length == 0)

                    {
                        string name = mi.Name;

                        mi.Invoke(Notify, null);

                    }
                    else
                    {
                        mi.Invoke(Notify, new object[] { propertyName });
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
                    RaiseProperty(prefix + "." + args.PropertyName);
                };
                obj.PropertyChanged += handler;

                if (_watch.ContainsKey(obj))
                {

                }

                _watch.Add(obj, handler);
            }
        }

        /// <summary>
        /// Stop wayching for that obj
        /// </summary>
        /// <param name="obj"></param>
        public void UnWatch(INotifyPropertyChanged obj)
        {
            if (obj != null)
            {
                if (_watch.ContainsKey(obj))
                {
                    PropertyChangedEventHandler handler = _watch[obj];
                    obj.PropertyChanged -= handler;
                    _watch.Remove(obj);
                }
            }
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

            if (_watchCollection.ContainsKey(collection)) { UnWatch(collection); }

            foreach (T obj in collection) Watch(obj, prefix);

            NotifyCollectionChangedEventHandler handler = delegate (object sender, NotifyCollectionChangedEventArgs e)
            {
                if (e.OldItems != null)
                    foreach (INotifyPropertyChanged item in e.OldItems)
                    {
                        UnWatch(item);
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

        public void UnWatch<T>(ObservableCollection<T> collection) where T : INotifyPropertyChanged
        {
            if (collection == null) return;

            foreach (T obj in collection) UnWatch(obj);

            if (_watchCollection.ContainsKey(collection))
            {
                NotifyCollectionChangedEventHandler handler = _watchCollection[collection];
                collection.CollectionChanged -= handler;
                _watchCollection.Remove(collection);
            }
        }
    }
}
