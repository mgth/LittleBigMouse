using System;
using System.Linq;

namespace Hlab.Notify
{

    public class NotifierEntry
    {
        public readonly object Lock = new object();
        protected readonly Notifier Notifier;

        protected Func<object, object> Getter;

        public NotifierEntry(Notifier notifier, Func<object, object> getter)
        {
            Notifier = notifier;
            Getter = getter;
            Value = getter(null);
        }

        public T GetValue<T>() => (T)Value;
        //public object GetObjectValue() => Value;

        public bool Update()
        {
            return SetValue(Getter(Value));
        }

        public bool SetValue<T>(T value)
        {
            lock (Lock)
            {
                if (Equals(Value, value))
                {
                    return false;
                }

                if (value != null && Value != null && value.GetType().IsArray)
                {
                    var a1 = (Value as Array).Cast<object>().ToArray();
                    var a2 = (value as Array).Cast<object>().ToArray();

                    if (a1.Length == a2.Length)
                    {
                        var eq = !a1.Where((t, i) => !Equals(t, a2[i])).Any();
                        if (eq) return false;
                    }
                }
                Value = value;
                return true;
            }
        }

        protected object Value;
    }
}