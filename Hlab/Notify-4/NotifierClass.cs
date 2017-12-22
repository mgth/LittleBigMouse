using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Reflection;

namespace HLab.Notify
{
    public class NotifierClass
    {
        public Type ClassType { get; }

        public ConcurrentDictionary<string,NotifierProperty> PropertiesByName = new ConcurrentDictionary<string, NotifierProperty>();
        public ConcurrentDictionary<PropertyInfo,NotifierProperty> Properties = new ConcurrentDictionary<PropertyInfo, NotifierProperty>();

        public NotifierClass(Type classType)
        {
            ClassType = classType;
        }

        public NotifierProperty GetProperty(PropertyInfo property) =>
            Properties.GetOrAdd(property, p => new NotifierPropertyReflexion(ClassType, p));
        

        public NotifierProperty GetProperty(string name) => PropertiesByName.GetOrAdd(name, n =>
        {
            var property = ClassType.GetProperty(n);
            return property == null ? new NotifierProperty(ClassType, n) : GetProperty(property);
        });
    }
}