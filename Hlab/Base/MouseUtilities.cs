using System;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Media;

namespace Hlab.Base
{
	/// <summary>
	/// Provides access to the mouse location by calling unmanaged code.
	/// </summary>
	/// <remarks>
	/// This class was written by Dan Crevier (Microsoft).  
	/// http://blogs.msdn.com/llobo/archive/2006/09/06/Scrolling-Scrollviewer-on-Mouse-DragStart-at-the-boundaries.aspx
	/// </remarks>
	public class MouseUtilities
	{
		[StructLayout( LayoutKind.Sequential )]
		private struct Win32Point
		{
			public Int32 X;
			public Int32 Y;
		};

		[DllImport( "user32.dll" )]
		private static extern bool GetCursorPos( ref Win32Point pt );

		[DllImport( "user32.dll" )]
		private static extern bool ScreenToClient( IntPtr hwnd, ref Win32Point pt );

		/// <summary>
		/// Returns the mouse cursor location.  This method is necessary during 
		/// a drag-drop operation because the WPF mechanisms for retrieving the
		/// cursor coordinates are unreliable.
		/// </summary>
		/// <param name="relativeTo">The Visual to which the mouse coordinates will be relative.</param>
		public static Point GetMousePosition( Visual relativeTo )
		{
			var mouse = new Win32Point();
			GetCursorPos( ref mouse );

			// Using PointFromScreen instead of Dan Crevier's code (commented out below)
			// is a bug fix created by William J. Roberts.  Read his comments about the fix
			// here: http://www.codeproject.com/useritems/ListViewDragDropManager.asp?msg=1911611#xx1911611xx
			return relativeTo.PointFromScreen( new Point( (double)mouse.X, (double)mouse.Y ) );

			#region Commented Out
			//System.Windows.Interop.HwndSource presentationSource =
			//    (System.Windows.Interop.HwndSource)PresentationSource.FromVisual( relativeTo );
			//ScreenToClient( presentationSource.Handle, ref mouse );
			//GeneralTransform transform = relativeTo.TransformToAncestor( presentationSource.RootVisual );
			//Point offset = transform.Transform( new Point( 0, 0 ) );
			//return new Point( mouse.X - offset.X, mouse.Y - offset.Y );
			#endregion // Commented Out
		}
	}
}