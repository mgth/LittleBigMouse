namespace System.Runtime.InteropServices
{
	internal static class InteropUtil
	{
		internal const int cbBuffer = 256;

		public static T ToStructure<T>(IntPtr ptr)
		{
			return (T)Marshal.PtrToStructure(ptr, typeof(T));
		}

		public static IntPtr StructureToPtr(object value)
		{
			IntPtr ret = Marshal.AllocHGlobal(Marshal.SizeOf(value));
			Marshal.StructureToPtr(value, ret, false);
			return ret;
		}

		public static void AllocString(ref IntPtr ptr, ref uint size)
		{
			FreeString(ref ptr, ref size);
			if (size == 0) size = cbBuffer;
			ptr = Marshal.AllocHGlobal(cbBuffer);
		}

		public static void FreeString(ref IntPtr ptr, ref uint size)
		{
			if (ptr != IntPtr.Zero)
			{
				Marshal.FreeHGlobal(ptr);
				ptr = IntPtr.Zero;
				size = 0;
			}
		}

		public static string GetString(IntPtr pString)
		{
			return Marshal.PtrToStringUni(pString);
		}

		public static bool SetString(ref IntPtr ptr, ref uint size, string value = null)
		{
			string s = GetString(ptr);
			if (value == string.Empty) value = null;
			if (string.Compare(s, value) != 0)
			{
				FreeString(ref ptr, ref size);
				if (value != null)
				{
					ptr = Marshal.StringToHGlobalUni(value);
					size = (uint)value.Length + 1;
				}
				return true;
			}
			return false;
		}

		public static T[] ToArray<S, T>(IntPtr ptr, int count) where S : IConvertible
		{
			IntPtr tempPtr;
			T[] ret = new T[count];
			int stSize = Marshal.SizeOf(typeof(S));
			for (int i = 0; i < count; i++)
			{
				tempPtr = new IntPtr(ptr.ToInt64() + (i * stSize));
				S val = ToStructure<S>(tempPtr);
				ret[i] = (T)Convert.ChangeType(val, typeof(T));
			}
			return ret;
		}

		public static T[] ToArray<T>(IntPtr ptr, int count) where T : IConvertible
		{
			return ToArray<T, T>(ptr, count);
		}
	}
}
