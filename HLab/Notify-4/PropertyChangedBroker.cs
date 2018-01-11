using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;

namespace HLab.Notify
{
    class PropertyChangedHolder
    {
        public event PropertyChangedEventHandler PropertyChanged;

        private object _oldValue = null;
        private readonly object _target = null;
        private readonly PropertyInfo _property;
        public PropertyChangedHolder(object target, string property)
        {
            _target = target;
            _property = target.GetType().GetProperty(property);
            if(_property==null)
            { }
        }

        public virtual void OnPropertyChanged(object sender, PropertyChangedEventArgs args)
        {
            var newValue = _property.GetValue(sender);
            PropertyChanged?.Invoke(_target, new NotifierPropertyChangedEventArgs(args.PropertyName,_oldValue,newValue));
            _oldValue = newValue;
        }
        public object Value() => _property.GetValue(_target);

        public object OldValue() => _oldValue;
    }

    public static class PropertyChangedBrokerExt
    {
        private static readonly ConditionalWeakTable<INotifyPropertyChanged, PropertyChangedBroker> Wtable = new ConditionalWeakTable<INotifyPropertyChanged, PropertyChangedBroker>();
        public static PropertyChangedBroker GetBroker(this INotifyPropertyChanged target)
        {
            return Wtable.GetValue(target, t => new PropertyChangedBroker(t));
        }
    }

    public class PropertyChangedBroker
    {

        private readonly ConcurrentDictionary<string, PropertyChangedHolder> _holders = new ConcurrentDictionary<string, PropertyChangedHolder>();
        private readonly INotifyPropertyChanged _target;

        public PropertyChangedBroker(INotifyPropertyChanged target)
        {
            _target = target;
            target.PropertyChanged += Target_PropertyChanged;
        }

        private PropertyChangedHolder GetHolder(string propertyName)
        {
            return _holders.GetOrAdd(propertyName, n => new PropertyChangedHolder(_target,n));
        }

        private void Target_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            if(_holders.TryGetValue(e.PropertyName,out var holder)) holder.OnPropertyChanged(sender,e);
        }

        public object Subscribe(string propertyName, PropertyChangedEventHandler handler)
        {
            var holder = GetHolder(propertyName);
            var value = holder.Value();
            holder.PropertyChanged += handler;

            return value;
        }

        public void UnSubscribe(string propertyName, PropertyChangedEventHandler handler)
        {
            if(_holders.TryGetValue(propertyName,out var holder))
                holder.PropertyChanged -= handler;
        }
    }
}
