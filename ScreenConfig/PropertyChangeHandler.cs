using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Linq.Expressions;
using System.Reflection;
using System.Runtime.CompilerServices;

namespace LbmScreenConfig
{
    public class PropertyChangeHandler
    {
        public event PropertyChangedEventHandler PropertyChanged;
        private readonly object _object;
        public PropertyChangeHandler(object obj)
        {
            _object = obj;
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

            foreach (string s in tmp)
            {
                //var handler = PropertyChanged;
                PropertyChanged?.Invoke(_object, new PropertyChangedEventArgs(s));
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

        static private readonly Dictionary<Type,Dictionary<string,List<string>>> _dictProperties = new Dictionary<Type, Dictionary<string, List<string>>>();
        static private readonly Dictionary<Type, Dictionary<string, List<MethodInfo>>> _dictMethods = new Dictionary<Type, Dictionary<string, List<MethodInfo>>>();
        public void RaiseProperty([CallerMemberName] string propertyName = null)
        {
            //            OnPropertyChanged(propertyName);
            Dictionary<string, List<string>> dictProperties;
            Dictionary<string, List<MethodInfo>> dictMethods;
            List<string> listProperties;
            List<MethodInfo> listMethods;

            if (!_dictProperties.TryGetValue(_object.GetType(), out dictProperties))
            {
                dictProperties = new Dictionary<string, List<string>>();
                _dictProperties.Add(_object.GetType(), dictProperties);
            }

            if (!_dictMethods.TryGetValue(_object.GetType(), out dictMethods))
            {
                dictMethods = new Dictionary<string, List<MethodInfo>>();
                _dictMethods.Add(_object.GetType(), dictMethods);
            }

            if (!dictProperties.TryGetValue(propertyName, out listProperties))
            {
                listProperties = new List<string>();
                listMethods = new List<MethodInfo>();

                GetDependOn(propertyName, ref listProperties, ref listMethods);
                dictProperties.Add(propertyName, listProperties);
                dictMethods.Add(propertyName, listMethods);
            }
            else dictMethods.TryGetValue(propertyName, out listMethods);

            foreach (var s in listProperties.Where(s => !s.Contains('.')))
                OnPropertyChanged(s);

            if (listMethods!=null)
                foreach (var mi in listMethods)
                {
                   mi.Invoke(_object, null);
                }
        }

        public void Watch(INotifyPropertyChanged obj, string prefix)
        {
            obj.PropertyChanged += delegate (object sender, PropertyChangedEventArgs args) { RaiseProperty(prefix + "." + args.PropertyName); };
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
