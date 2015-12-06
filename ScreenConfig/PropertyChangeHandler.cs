using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Runtime.CompilerServices;

namespace LbmScreenConfig
{
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
                    catch (Exception ex)
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
        bool SetProperty<T, TProperty>(T input, Expression<Func<Screen, TProperty>> outExpr,
            [CallerMemberName] string propertyName = null)
        {
            //if (string.IsNullOrEmpty(input)) return true;
            var expr = (MemberExpression)outExpr.Body;
            var prop = (PropertyInfo)expr.Member;
            if (prop.Equals(input)) return false;

            prop.SetValue(this, input, null);
            return true;
        }

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
            foreach (var s in listProperties.Where(s => !s.Contains('.')))
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
