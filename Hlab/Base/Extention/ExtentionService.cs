using System;
using System.Runtime.CompilerServices;

namespace Hlab.Base.Extention
{
    public class Extends : Attribute
    {
        public Extends(Type type)
        {
            Type = type;
        }

        public Type Type { get; }

    }
    public class ExtentionInstance
    {
    }


    public class ExtentionClass
    {
        private ConditionalWeakTable<object, ExtentionInstance> _wTable = new ConditionalWeakTable<object,ExtentionInstance>();
    }
    public class ExtentionService : Singleton<ExtentionService>
    {
    }
}
