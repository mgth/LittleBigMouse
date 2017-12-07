namespace Hlab.Base
{
    public static class StringExt
    {
        public static bool Like(this string s, string pattern)
        {
            string[] parts = pattern.Split('*');
            if (!s.StartsWith(parts[0])) return false;

            s = s.Remove(0,parts[0].Length);
            if (s == "") return true;

            for (int i = 1; i < parts.Length-1; i++)
            {
                int pos = s.IndexOf(parts[i]);
                if (pos == -1) return false;

                s = s.Remove(0, pos+parts[i].Length);
            }

            string end = parts[parts.Length - 1];

            if (s.EndsWith(end)) return true;
            return false;
        }
    }
}
