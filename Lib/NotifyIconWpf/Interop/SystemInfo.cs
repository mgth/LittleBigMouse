using System.Windows.Interop;

namespace Hardcodet.Wpf.TaskbarNotification.Interop
{
    /// <summary>
    /// This class is a helper for system information, currently to get the DPI factors
    /// </summary>
    public static class SystemInfo
    {
        private static readonly System.Windows.Point DpiFactors;

        static SystemInfo()
        {
            using (var source = new HwndSource(new HwndSourceParameters()))
            {
                if (source.CompositionTarget?.TransformToDevice != null)
                {
                    DpiFactors = new System.Windows.Point(source.CompositionTarget.TransformToDevice.M11, source.CompositionTarget.TransformToDevice.M22);
                    return;
                }
                DpiFactors = new System.Windows.Point(1, 1);
            }
        }

        /// <summary>
        /// Returns the DPI X Factor
        /// </summary>
        public static double DpiFactorX => DpiFactors.X;

        /// <summary>
        /// Returns the DPI Y Factor
        /// </summary>
        public static double DpiFactorY => DpiFactors.Y;
    }
}