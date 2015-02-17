using System;

namespace Microsoft.Win32.TaskScheduler
{
	internal class CultureSwitcher : IDisposable
	{
		System.Globalization.CultureInfo cur, curUI;

		public CultureSwitcher(System.Globalization.CultureInfo culture)
		{
			cur = System.Threading.Thread.CurrentThread.CurrentCulture;
			curUI = System.Threading.Thread.CurrentThread.CurrentUICulture;
			System.Threading.Thread.CurrentThread.CurrentCulture = System.Threading.Thread.CurrentThread.CurrentUICulture = culture;
		}

		public void Dispose()
		{
			System.Threading.Thread.CurrentThread.CurrentCulture = cur;
			System.Threading.Thread.CurrentThread.CurrentUICulture = curUI;
		}
	}
}
