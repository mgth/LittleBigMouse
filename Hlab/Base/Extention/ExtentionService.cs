/*
  HLab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Base.

    HLab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Base is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Runtime.CompilerServices;

namespace HLab.Base.Extention
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
