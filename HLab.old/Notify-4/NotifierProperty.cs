using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;
using System.Threading.Tasks;

namespace HLab.Notify
{
    public class NotifierProperty
    {
        public Type ClassType { get; }
        public string Name { get; }

        public NotifierProperty(Type classType, string name)
        {
            Name = name;
            ClassType = classType;
        }
    }
    public class NotifierPropertyReflexion : NotifierProperty
    {
        public PropertyInfo Property { get; }

        public NotifierPropertyReflexion(Type classType, PropertyInfo property):base(classType,property.Name)
        {
            Property = property;
        }
    }
}
