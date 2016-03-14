using System;

namespace NotifyChange
{
    public class DependsOn : Attribute
    {
        public DependsOn(params string[] dp)
        {
            Properties = dp;
        }

        public string[] Properties { get; }
    }
}
