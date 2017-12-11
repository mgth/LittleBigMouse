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

    [AttributeUsage(AttributeTargets.Property, AllowMultiple = false)]
    public class NotifierType : Attribute
    {
        private readonly Type _type;
        public NotifierType(Type type)
        {
            _type = type;
        }

        public NotifierEntry GetEntry()
        {
            var entry = (NotifierEntry)Activator.CreateInstance(_type);


            return entry;
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
