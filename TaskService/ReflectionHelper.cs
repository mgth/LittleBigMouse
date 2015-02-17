namespace System.Reflection
{
	internal static class ReflectionHelper
	{
		public static Type LoadType(string typeName, string asmRef)
		{
			Type ret = null;
			if (!TryGetType(Assembly.GetCallingAssembly(), typeName, ref ret))
				if (!TryGetType(asmRef, typeName, ref ret))
					if (!TryGetType(Assembly.GetExecutingAssembly(), typeName, ref ret))
						TryGetType(Assembly.GetEntryAssembly(), typeName, ref ret);
			return ret;
		}

		private static bool TryGetType(string asmRef, string typeName, ref Type type)
		{
			try
			{
				return TryGetType(Assembly.LoadFrom(asmRef), typeName, ref type);
			}
			catch { }
			return false;
		}

		private static bool TryGetType(Assembly asm, string typeName, ref Type type)
		{
			if (asm != null)
			{
				try
				{
					type = asm.GetType(typeName, false, false);
					return (type != null);
				}
				catch { }
			}
			return false;
		}

		public static T InvokeMethod<T>(Type type, string methodName, params object[] args)
		{
			object o = Activator.CreateInstance(type);
			return InvokeMethod<T>(o, methodName, args);
		}

		public static T InvokeMethod<T>(Type type, object[] instArgs, string methodName, params object[] args)
		{
			object o = Activator.CreateInstance(type, instArgs);
			return InvokeMethod<T>(o, methodName, args);
		}

		public static T InvokeMethod<T>(object obj, string methodName, params object[] args)
		{
			Type[] argTypes = (args == null || args.Length == 0) ? Type.EmptyTypes : Array.ConvertAll<object, Type>(args, delegate(object o) { return o == null ? typeof(object) : o.GetType(); });
			return InvokeMethod<T>(obj, methodName, argTypes, args);
		}

		public static T InvokeMethod<T>(object obj, string methodName, Type[] argTypes, object[] args)
		{
			if (obj != null)
			{
				MethodInfo mi = obj.GetType().GetMethod(methodName, argTypes);
				if (mi != null)
					return (T)Convert.ChangeType(mi.Invoke(obj, args), typeof(T));
			}
			return default(T);
		}
	}
}
