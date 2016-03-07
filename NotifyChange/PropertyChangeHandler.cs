using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Windows;

namespace NotifyChange
{
    public class Notifier : DependencyObject, INotifyPropertyChanged
    {
        public Notifier()
        {
            Init();
        }

        public event PropertyChangedEventHandler PropertyChanged;

        protected override void OnPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            base.OnPropertyChanged(e);

            RaiseProperty(e.Property.Name);
        }

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
                        sink(this, arg);
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
        //bool SetProperty<T, TProperty>(T input, Expression<Func<Screen, TProperty>> outExpr,
        //    [CallerMemberName] string propertyName = null)
        //{
        //    //if (string.IsNullOrEmpty(input)) return true;
        //    var expr = (MemberExpression)outExpr.Body;
        //    var prop = (PropertyInfo)expr.Member;
        //    if (prop.Equals(input)) return false;

        //    prop.SetValue(this, input, null);
        //    return true;
        //}

        private void Init()
        {
            Type t = GetType();
            while (t != null)
            {
                MethodInfo[] m = t.GetMethods(
                    BindingFlags.DeclaredOnly |
                    BindingFlags.Instance | BindingFlags.Public
                    | BindingFlags.NonPublic
                    );
                foreach (MethodInfo mi in m)
                {
                    if(mi.GetCustomAttributes(false).OfType<DependsOn>().Any())
                    {
                        if (mi.GetParameters().Length == 0)
                        {
                            mi.Invoke(this, null);

                        }
                        else
                        {
                            mi.Invoke(this, new object[] { "" });
                        }
                    }
                }

                t = t.BaseType;
            }
        }

        private void GetDependOn(string propertyName, ref List<string> list, ref List<MethodInfo> listMethods)
        {
            list.Add(propertyName);

            PropertyInfo[] props = GetType().GetProperties();
            foreach (PropertyInfo pInfo in props)
            {
                foreach (DependsOn ca in
                    pInfo.GetCustomAttributes(false).OfType<DependsOn>())
                {
                    if (ca.Properties != null)
                    {
                        if ( (ca.Properties.Contains(propertyName.Split('.')[0]) || ca.Properties.Contains(propertyName) )  && !list.Contains(pInfo.Name))
                        {
                            GetDependOn(pInfo.Name, ref list, ref listMethods);
                        }
                    }
                }
            }

            Type t = GetType();
            while (t != null)
            {
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

        private static readonly object LockDict = new object();

        public void RaiseProperty([CallerMemberName] string propertyName = null)
        {
            Dictionary<string, List<string>> dictClassProperties;
            Dictionary<string, List<MethodInfo>> dictClassMethods;
            List<string> listProperties;
            List<MethodInfo> listMethods;

            Type type = GetType();
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

                //            if (!dictProperties.TryGetValue(propertyName, out listProperties))
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

                        mi.Invoke(this, null);

                    }
                    else
                    {
                        mi.Invoke(this, new object[] {propertyName});
                    }
                }
        }
        public void Watch(ref EventHandler evt, string prefix)
        {
            evt += delegate
            {
                RaiseProperty(prefix);
            };
        }

        public void Watch(ref SizeChangedEventHandler evt, string prefix)
        {
            evt += delegate
            {
                RaiseProperty(prefix);
            };
        }

        public void Watch(INotifyPropertyChanged obj, string prefix)
        {
            if (obj != null)
            {
                PropertyChangedEventHandler handler = delegate (object sender, PropertyChangedEventArgs args)
                {
                    RaiseProperty(prefix + "." + args.PropertyName);
                };
                obj.PropertyChanged += handler;
                _watch.Add(obj,handler);
            }
            
        }

        public void UnWatch(INotifyPropertyChanged obj)
        {
            if (obj != null)
            {
                if (_watch.ContainsKey(obj))
                {
                    PropertyChangedEventHandler handler =_watch[obj];
                    obj.PropertyChanged -= handler;
                    _watch.Remove(obj);                 
                }
                // TODO : should obj not exist ?
            }
        }

        public void Watch<T>(ObservableCollection<T> collection, string prefix) where T:INotifyPropertyChanged
        {
            if (collection == null) return;

            foreach(T obj in collection) Watch(obj, prefix);

            collection.CollectionChanged += delegate(object sender, NotifyCollectionChangedEventArgs e)
            {
                if (e.OldItems!=null)
                foreach (INotifyPropertyChanged item in e.OldItems)
                {
                    UnWatch(item);
                    RaiseProperty(prefix);
                }

                if (e.NewItems!=null)
                foreach (INotifyPropertyChanged item in e.NewItems)
                {
                    Watch(item , prefix);
                    RaiseProperty(prefix);
                }
            };
        }

        public void UnWatch<T>(ObservableCollection<T> collection, string prefix) where T : INotifyPropertyChanged
        {
            if (collection != null)
                collection.CollectionChanged -= delegate (object sender, NotifyCollectionChangedEventArgs e)
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
        }
        protected static PropertyMetadata WatchNotifier() => new PropertyMetadata(null, (d, e) =>
        {
            (d as Notifier)?.UnWatch(e.OldValue as Notifier);
            (d as Notifier)?.Watch(e.NewValue as Notifier, e.Property.Name);
        });

    }
    public class PropertyChangedHelper
    {
        public event PropertyChangedEventHandler PropertyChanged;

        private object _object;

        public PropertyChangedHelper(INotifyPropertyChanged obj)
        {
            _object = obj;
        }

        public void Add (INotifyPropertyChanged obj, PropertyChangedEventHandler handler)
        {
            _object = obj; PropertyChanged += handler; }
        public void Remove (PropertyChangedEventHandler handler) { PropertyChanged -= handler; }

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
                    PropertyChangedEventHandler sink = (PropertyChangedEventHandler) del;
                    try
                    {
                        sink(_object, arg);
                    }
                    catch (Exception)
                    {
                    
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
        //bool SetProperty<T, TProperty>(T input, Expression<Func<Screen, TProperty>> outExpr,
        //    [CallerMemberName] string propertyName = null)
        //{
        //    //if (string.IsNullOrEmpty(input)) return true;
        //    var expr = (MemberExpression)outExpr.Body;
        //    var prop = (PropertyInfo)expr.Member;
        //    if (prop.Equals(input)) return false;

        //    prop.SetValue(this, input, null);
        //    return true;
        //}

        private void GetDependOn(string propertyName, ref List<string> list, ref List<MethodInfo> listMethods)
        {
            list.Add(propertyName);

            PropertyInfo[] props = _object.GetType().GetProperties();
            foreach (PropertyInfo pInfo in props)
            {
                foreach (DependsOn ca in
                    pInfo.GetCustomAttributes(false).OfType<DependsOn>())
                {
                    if (ca.Properties != null)
                    {
                        if (ca.Properties.Contains(propertyName) && !list.Contains(pInfo.Name))
                        {
                            GetDependOn(pInfo.Name,ref list, ref listMethods);
                        }
                    }
                }
            }

            MethodInfo[] methods = _object.GetType().GetMethods();
            foreach (MethodInfo mInfo in methods)
            {
                foreach (DependsOn ca in
                    mInfo.GetCustomAttributes(false).OfType<DependsOn>())
                {
                    if (ca.Properties != null)
                    {
                        if (ca.Properties.Contains(propertyName) && !listMethods.Contains(mInfo))
                        {
                            listMethods.Add(mInfo);
                        }
                    }
                }
            }
        }

        private static readonly Dictionary<Type,Dictionary<string,List<string>>> _dictProperties = new Dictionary<Type, Dictionary<string, List<string>>>();
        private static readonly Dictionary<Type, Dictionary<string, List<MethodInfo>>> _dictMethods = new Dictionary<Type, Dictionary<string, List<MethodInfo>>>();

        private static readonly object LockDict = new object();

        public void RaiseProperty([CallerMemberName] string propertyName = null)
        {
            if (_object == null) return;

            Dictionary<string, List<string>> dictClassProperties;
            Dictionary<string, List<MethodInfo>> dictClassMethods;
            List<string> listProperties;
            List<MethodInfo> listMethods;

            Type Type = _object.GetType();
            lock (LockDict)
            {

                if (!_dictProperties.TryGetValue(Type, out dictClassProperties))
                {
                    dictClassProperties = new Dictionary<string, List<string>>();
                    _dictProperties.Add(Type, dictClassProperties);
                }

                if (!_dictMethods.TryGetValue(Type, out dictClassMethods))
                {
                    dictClassMethods = new Dictionary<string, List<MethodInfo>>();
                    _dictMethods.Add(Type, dictClassMethods);
                }

            //            if (!dictProperties.TryGetValue(propertyName, out listProperties))
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
            foreach (var s in listProperties)//.Where(s => !s.Contains('.')))
                OnPropertyChanged(s);

            //Execute all methods marked with DependsOn
            if (listMethods!=null)
                foreach (var mi in listMethods)
                {
                   mi.Invoke(_object, null);
                }
        }
        public void Watch(ref EventHandler evt, string prefix)
        {
            evt += delegate 
            {
                RaiseProperty(prefix);
            };
        }


        public void Watch(INotifyPropertyChanged obj, string prefix)
        {
            obj.PropertyChanged += delegate(object sender, PropertyChangedEventArgs args)
            {
                RaiseProperty(prefix + "." + args.PropertyName);
            };
        }
        public void UnWatch(INotifyPropertyChanged obj, string prefix)
        {
            obj.PropertyChanged -= delegate (object sender, PropertyChangedEventArgs args)
            {
                RaiseProperty(prefix + "." + args.PropertyName);
            };
        }


    }
    public class DependsOn : Attribute
    {
        public DependsOn(params string[] dp)
        {
            Properties = dp;
        }

        public string[] Properties { get; }
    }
}
