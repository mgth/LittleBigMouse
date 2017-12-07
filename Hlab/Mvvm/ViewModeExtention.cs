using System;
using System.Windows.Markup;

namespace Hlab.Mvvm
{


        [MarkupExtensionReturnType(typeof(Type))]
    public class ViewModeExtention : MarkupExtension
    {
        public string ViewModeName { get; set; }
        public ViewModeExtention(string name)
        {
            ViewModeName = name;
        }
        public override object ProvideValue(IServiceProvider serviceProvider)
        {
            if (string.IsNullOrEmpty(ViewModeName))
            {
                throw new ArgumentException("The variable name can't be null or empty");
            }

            return Type.GetType(ViewModeName);
        }
    }
}
