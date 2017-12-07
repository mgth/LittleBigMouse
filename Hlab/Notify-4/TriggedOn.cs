using System;
using System.Collections.Generic;

namespace Hlab.Notify
{
    public class PropertyNotReady : Exception
    {
        public object ReturnValue { get; }
        public PropertyNotReady(object returnValue)
        {
            ReturnValue = returnValue;
        }
    }


    [AttributeUsage(AttributeTargets.Property | AttributeTargets.Method, AllowMultiple = true)]
    public class TriggedOn : Attribute
    {
        public TriggedOn(string path="")
        {
            Pathes = path.Split('.');
        }
        public TriggedOn(params string[] paths)
        {
            var list = new List<string>();
            foreach (var s in paths)
            {
                list.AddRange(s.Split('.'));
            }
            Pathes = list;

        }

        public IList<string> Pathes { get; }

    }




}
