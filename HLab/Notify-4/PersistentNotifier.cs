using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;
using Microsoft.Win32;

namespace HLab.Notify
{
    public interface IRegistryStored
    {
        string Key { get; }
    }

    public class RegistryPersister : Persister
    {
        public RegistryPersister(INotifyPropertyChanged obj, string key) : base(obj)
        {
            Key = key;
        }

        public string Key { get; set; }

        protected override object Load(string propertyName)
        {
            if(string.IsNullOrEmpty(Key)) throw new PersisterLoadException();
            var none = new object();

            var key = Registry.CurrentUser.OpenSubKey(Key, false);
            if(key==null) throw new PersisterLoadException();

            var value = key.GetValue(propertyName,none);
            if (ReferenceEquals(value,none) )
                throw new PersisterLoadException();

            return value;
        }

        protected override void Save(string entry, object value)
        {
            if(value==null)
                try{Registry.CurrentUser.OpenSubKey(Key,true)?.DeleteValue(entry);}
                catch (ArgumentException) { }
            else
                Registry.CurrentUser.CreateSubKey(Key,true).SetValue(entry,value);
        }
    }


    public class PersisterLoadException : Exception
    {
        
    }

    public class Persister
    {
        protected readonly ConcurrentBag<PropertyInfo> Dirty = new ConcurrentBag<PropertyInfo>();
        public bool IsDirty => Dirty.Count > 0;

        public bool Loading { get; private set; } = false;

        private readonly object _source;
        public Persister(INotifyPropertyChanged obj)
        {
            _source = obj;
            foreach (var property in _source.GetType().GetProperties())
            {
                foreach (var unused in property.GetCustomAttributes().OfType<Persistent>())
                {
                    Dirty.Add(property);
                    //switch (attr.Persistency)
                    //{
                    //    case Persistency.OnChange:
                    //        Save(property);
                    //        break;
                    //    case Persistency.OnSave:
                    //        Dirty.Add(property);
                    //        break;
                    //    default:
                    //        throw new ArgumentOutOfRangeException();
                    //}
                }
            }
            obj.PropertyChanged += Obj_PropertyChanged;
        }

        private void Obj_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            var property = _source.GetType().GetProperty(e.PropertyName);
            if (property == null) return;

            foreach (var attr in property.GetCustomAttributes().OfType<Persistent>())
            {
                switch (attr.Persistency)
                {
                    case Persistency.OnChange:
                        Save(property);
                        break;
                    case Persistency.OnSave:
                        Dirty.Add(property);
                        break;
                    default:
                        throw new ArgumentOutOfRangeException();
                }
            }
        }

        public virtual void Save()
        {
            while (!Dirty.IsEmpty)
            {
                if (Dirty.TryTake(out var e))
                {
                    Save(e);
                }
            }
        }

        public virtual void Load()
        {
            Loading = true;

            foreach (var property in _source.GetType().GetProperties())
            {
                foreach (var unused in property.GetCustomAttributes().OfType<Persistent>())
                {
                    Load(property);
                }
            }
            while(Dirty.TryTake(out var unused2));

            Loading = false;
        }

        protected void Load(PropertyInfo property)
        {
            try
            {
                var value = Load(property.Name);
                if (value.GetType() == property.PropertyType)
                {
                    property.SetValue(_source, value);
                }

                if (property.PropertyType == typeof(bool))
                {
                    property.SetValue(_source, value.ToString()=="1");
                }
                
            }
            catch (PersisterLoadException)
            { }
        }

        protected virtual object Load(string propertyName)
        {
            throw new PersisterLoadException();
        }

        protected void Save(PropertyInfo property)
        {
            Save(property.Name, property.GetValue(_source));
        }

        protected void Save(string entry)
        {
            Save(GetType().GetProperty(entry));
        }

        protected virtual void Save(string entry, object value)
        { }

    }


}
