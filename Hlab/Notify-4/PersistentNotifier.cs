using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace Hlab.Notify
{
    public interface IRegistryStored
    { }

    public class RegistryNotifier : PersistentNotifier
    {
        public static void Register()
        {
            NotifierService.D.Factory.Register<IRegistryStored>((o)=>new RegistryNotifier(o.GetType()));    
        }

        protected override bool Save(string propertyName, NotifierEntry entry)
        {
            return true;
        }

        public RegistryNotifier(Type classType) : base(classType)
        {
        }
    }


    public abstract class PersistentNotifier : Notifier
    {
        protected readonly ConcurrentDictionary<string,NotifierEntry> Dirty = new ConcurrentDictionary<string,NotifierEntry>();
        public bool IsDirty => Dirty.Count > 0;

        public virtual void Save()
        {
            foreach (var entry in Dirty)
            {
                Save(entry.Key, entry.Value);
            }
            Dirty.Clear();
        }

        protected abstract bool Save(string propertyName, NotifierEntry entry);

        protected PersistentNotifier(Type classType) : base(classType)
        {
        }
    }


}
