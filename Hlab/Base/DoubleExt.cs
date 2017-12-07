namespace Hlab.Base
{
    public static class DoubleExt
    {
        public static bool IsRegular(this double d)
        {
            if (double.IsNaN(d)) return false;
            return !double.IsInfinity(d);
        }
    }
}
