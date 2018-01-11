/*
  HLab.Notify.4
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Notify.4.

    HLab.Notify.4 is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Notify.4 is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Linq;
using System.Reflection;

namespace HLab.Notify
{

    public class NotifierEntry
    {
        public readonly object Lock = new object();
        protected readonly Notifier Notifier;

        protected Func<object, object> Getter;

        public NotifierEntry(Notifier notifier, NotifierProperty property, Func<object, object> getter)
        {
            Notifier = notifier;
            Property = property;
            Getter = getter;
            Value = getter(null);
        }

        public NotifierProperty Property { get; }

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

                Property.AddOneToMany(Value,value,Notifier.Target);

                Value = value;
                return true;
            }
        }

        protected object Value;
    }

}