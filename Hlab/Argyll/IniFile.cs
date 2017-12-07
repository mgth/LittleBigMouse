using System.Runtime.InteropServices;
using System.Text;

namespace Hlab.Argyll
{
    /// <summary>
    /// Create a New INI file to store or load data
    /// </summary>
    public class IniFile
    {
        public string Path;

        [DllImport("kernel32")]
        private static extern long WritePrivateProfileString(string section,
            string key, string val, string filePath);
        [DllImport("kernel32")]
        private static extern int GetPrivateProfileString(string section,
                 string key, string def, StringBuilder retVal,
            int size, string filePath);

        /// <summary>
        /// INIFile Constructor.
        /// </summary>
        /// <PARAM name="iniPath"></PARAM>
        public IniFile(string iniPath)
        {
            Path = iniPath;
        }
        /// <summary>
        /// Write Data to the INI File
        /// </summary>
        /// <PARAM name="section"></PARAM>
        /// section name
        /// <PARAM name="key"></PARAM>
        /// key Name
        /// <PARAM name="value"></PARAM>
        /// value Name
        public void WriteValue(string section, string key, string value)
        {
            WritePrivateProfileString(section, key, value, this.Path);
        }

        /// <summary>
        /// Read Data value From the Ini File
        /// </summary>
        /// <PARAM name="section"></PARAM>
        /// <PARAM name="key"></PARAM>
        /// <PARAM name="Path"></PARAM>
        /// <returns></returns>
        public string ReadValue(string section, string key, string def)
        {
            StringBuilder temp = new StringBuilder(255);
            int i = GetPrivateProfileString(section, key, def, temp,
                                            255, this.Path);
            return temp.ToString();

        }
    }
}