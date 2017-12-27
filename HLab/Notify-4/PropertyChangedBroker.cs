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
        private readonly PropertyInfo _property;
        public PropertyChangedHolder(object target, string property)
        {
            _property = target.GetType().GetProperty(property);
        }

        public virtual void OnPropertyChanged(object sender, PropertyChangedEventArgs args)
        {
            var newValue = _property.GetValue(sender);
            PropertyChanged?.Invoke(sender, new NotifierPropertyChangedEventArgs(args.PropertyName,_oldValue,newValue));
            _oldValue = newValue;
        }
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

        private void Target_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            if(_holders.TryGetValue(e.PropertyName,out var holder)) holder.OnPropertyChanged(sender,e);
        }

        public void Subscribe(string propertyName, PropertyChangedEventHandler handler)
        {
            var holder = _holders.GetOrAdd(propertyName, n => new PropertyChangedHolder(_target,n));
            holder.PropertyChanged += handler;
        }
        public void UnSubscribe(string propertyName, PropertyChangedEventHandler handler)
        {
            if(_holders.TryGetValue(propertyName,out var holder))
                holder.PropertyChanged -= handler;
        }
    }
}
