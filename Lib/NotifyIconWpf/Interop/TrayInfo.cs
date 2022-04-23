// Some interop code taken from Mike Marshall's AnyForm

using System.Drawing;

namespace Hardcodet.Wpf.TaskbarNotification.Interop
{
    /// <summary>
    /// Resolves the current tray position.
    /// </summary>
    public static class TrayInfo
    {
        /// <summary>
        /// Gets the position of the system tray.
        /// </summary>
        /// <returns>Tray coordinates.</returns>
        public static Point GetTrayLocation()
        {
            int space = 2;
            var info = new AppBarInfo();
            info.GetSystemTaskBarPosition();

            Rectangle rcWorkArea = info.WorkArea;

            int x = 0, y = 0;
            switch (info.Edge)
            {
                case AppBarInfo.ScreenEdge.Left:
                    x = rcWorkArea.Right + space;
                    y = rcWorkArea.Bottom;
                    break;
                case AppBarInfo.ScreenEdge.Bottom:
                    x = rcWorkArea.Right;
                    y = rcWorkArea.Bottom - rcWorkArea.Height - space;
                    break;
                case AppBarInfo.ScreenEdge.Top:
                    x = rcWorkArea.Right;
                    y = rcWorkArea.Top + rcWorkArea.Height + space;
                    break;
                case AppBarInfo.ScreenEdge.Right:
                    x = rcWorkArea.Right - rcWorkArea.Width - space;
                    y = rcWorkArea.Bottom;
                    break;
            }

            return GetDeviceCoordinates(new Point {X = x, Y = y});
        }

        /// <summary>
        /// Recalculates OS coordinates in order to support WPFs coordinate
        /// system if OS scaling (DPIs) is not 100%.
        /// </summary>
        /// <param name="point">Point</param>
        /// <returns>Point</returns>
        public static Point GetDeviceCoordinates(Point point)
        {
          return new Point
          {
              X = (int)(point.X / SystemInfo.DpiFactorX),
              Y = (int)(point.Y / SystemInfo.DpiFactorY)
          };
        }
    }
}