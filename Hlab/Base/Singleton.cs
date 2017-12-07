/*
  Hlab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of Hlab.Base.

    Hlab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    Hlab.Base is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Threading;

namespace Hlab.Base
{
    public abstract class Singleton<T>
        where T: Singleton<T>//, new()
    {
        // ReSharper disable once StaticMemberInGenericType
        private static bool _lazyCalled = false;
        private static readonly Lazy<T> Lazy =
            new Lazy<T>(() =>
            {
                _lazyCalled = true;
                T t = (T)Activator.CreateInstance(typeof(T),true);
                return t;
            });

        public static T D => Lazy.Value;

        protected Singleton()
        {
            if(!_lazyCalled || Lazy.IsValueCreated)
                throw new InvalidOperationException("Constructing a " + typeof(T).Name +
                " manually is not allowed, use the " + nameof(D) + " property.");
        }
    }

    public abstract class Singleton2<T>
        where T: Singleton2<T>, new()
    {
        private static T _instance;
        private static readonly ManualResetEvent InitEvent = new ManualResetEvent(false);

        static Singleton2()
        {
            ThreadPool.QueueUserWorkItem(state => Init());
        }

        public static T D
        {
            get
            {
                InitEvent.WaitOne();
                return _instance;                
            }
        }
        private static void Init()
        {
        _instance = new T();
        // long running code here...


        InitEvent.Set();
        }
    }

    public abstract class Singleton3<T>
        where T : Singleton3<T>//, new()
    {
        private static T _instance;
        private static object _lock = new object();

        public static T D
        {
            get
            {
                if (_instance != null) return _instance;
                lock(_lock)
                return _instance ?? ((T) Activator.CreateInstance(typeof(T), true));
            }
        }

        protected Singleton3()
        {
            if (_instance!=null)
                throw new InvalidOperationException("Constructing a " + typeof(T).Name +
                                                    " manually is not allowed, use the " + nameof(D) + " property.");

            _instance = (T)this;
        }
    }

}
