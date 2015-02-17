using System.Text.RegularExpressions;

namespace Microsoft.Win32.TaskScheduler
{
	/// <summary>
	/// Represents a wildcard running on the
	/// <see cref="System.Text.RegularExpressions"/> engine.
	/// </summary>
	public class Wildcard : Regex
	{
		/// <summary>
		/// Initializes a wildcard with the given search pattern and options.
		/// </summary>
		/// <param name="pattern">The wildcard pattern to match.</param>
		/// <param name="options">A combination of one or more <see cref="System.Text.RegularExpressions.RegexOptions"/>.</param>
		public Wildcard(string pattern, RegexOptions options = RegexOptions.Compiled | RegexOptions.IgnoreCase | RegexOptions.IgnorePatternWhitespace)
			: base(WildcardToRegex(pattern), options)
		{
		}

		/// <summary>
		/// Converts a wildcard to a regex.
		/// </summary>
		/// <param name="pattern">The wildcard pattern to convert.</param>
		/// <returns>A regex equivalent of the given wildcard.</returns>
		public static string WildcardToRegex(string pattern)
		{
			string s = "^" + Regex.Escape(pattern) + "$"; s = Regex.Replace(s, @"(?<!\\)\\\*", @".*"); // Negative Lookbehind
			s = Regex.Replace(s, @"\\\\\\\*", @"\*");
			s = Regex.Replace(s, @"(?<!\\)\\\?", @".");  // Negative Lookbehind
			s = Regex.Replace(s, @"\\\\\\\?", @"\?");
			return Regex.Replace(s, @"\\\\\\\\", @"\\"); 
		}
	}
}
