using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;

namespace HLab.Notify
{
    public class NotifierProperty
    {
        public NotifierClass Class { get; }
        public string Name { get; }

        public NotifierProperty(NotifierClass @class, string name)
        {
            Name = name;
            Class = @class;
        }

        private readonly ConditionalWeakTable<object, IList> _weakOneToMany = new ConditionalWeakTable<object, IList>();

        public virtual NotifierEntry GetNewEntry(Notifier notifier, Func<object, object> getter) => new NotifierEntry(notifier,this,getter);
        public void RegisterOneToMany(object target, IList list)
        {
            _weakOneToMany.Add(target, list);
        }

        public void AddOneToMany(object oldValue, object newValue, object target)
        {
            if (Name == "Group") { }

            if (oldValue!=null && _weakOneToMany.TryGetValue(oldValue, out var oldCollection))
            {
                oldCollection.Remove(target);
            }
            if (newValue!=null && _weakOneToMany.TryGetValue(newValue, out var newCollection))
            {
                newCollection.Add(target);
            }
        }
    }

    public class NotifierPropertyReflexion : NotifierProperty
    {
        public PropertyInfo Property { get; }

        public NotifierPropertyReflexion(NotifierClass @class, PropertyInfo property):base(@class,property.Name)
        {
            Property = property;
        }
    }


}
