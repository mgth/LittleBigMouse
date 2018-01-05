using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Xml;
using System.Xml.Xsl;
using HLab.Base;

namespace HLab.Mvvm.Icons
{
    class IconCache
    {
        private readonly object _lockCache = new object();
        private readonly Dictionary<string, UIElement> _cache = new Dictionary<string, UIElement>();
        private readonly Assembly _assembly;

        public IconCache(Assembly assembly)
        {
            _assembly = assembly;
        }

        public UIElement Get(string name, Func<Assembly, string, UIElement> f)
        {
            lock (_lockCache)
            {
                if (_cache.TryGetValue(name, out UIElement ui))
                {
                    return ui;
                }
                ui = f(_assembly, name);
                return ui;
            }
        }
    }


    public class IconService : Singleton<IconService>
    {
        private readonly ConditionalWeakTable<Assembly,IconCache> _cache = new ConditionalWeakTable<Assembly, IconCache>();

        public UIElement GetIcon(string name)
        {
            return GetIcon(Assembly.GetCallingAssembly(),name);
        }
        public UIElement GetIcon(string assemblyName, string name)
        {
            var assembly = AppDomain.CurrentDomain.GetAssemblies().
              SingleOrDefault(a => a.GetName().Name == assemblyName);

            return GetIcon(assembly, name);
        }
        public UIElement GetIcon(Assembly assembly, string name)
        {
            var cache = _cache.GetValue(assembly, a => new IconCache(a));

            return cache.Get(name,(a,n)=> GetIconXaml(a, n) ?? GetFromSvg(a, n));
        }
        public UIElement GetIconXaml(Assembly assembly, string name)
        {
            var uri = new Uri("/" + assembly.GetName().Name + ";component/Icons/"+ name + ".xaml", UriKind.RelativeOrAbsolute);

            try
            {
                var obj = Application.LoadComponent(uri);
                return (UIElement) obj;
            }
            catch (System.IO.IOException ex)
            {
                return GetFromSvg(assembly, name);
                return null;
            }
        }

        public UIElement GetFromSvg(Assembly assembly, string name)
        {
            string[] resources = assembly.GetManifestResourceNames();

            using (var xslStream = Assembly.GetAssembly(this.GetType()).GetManifestResourceStream("HLab.Mvvm.Icons.svg2xaml.xsl"))
            {
                if (xslStream == null) throw new IOException("xsl file not found");
                using (var stylesheet = XmlReader.Create(xslStream))
                {
                    var t = new XslCompiledTransform();

                    XsltSettings settings = new XsltSettings {EnableDocumentFunction = true};

                    t.Load(stylesheet, settings, new XmlUrlResolver());

                    using (var s = new MemoryStream())
                    {
                        using (
                            var svg =
                                assembly.GetManifestResourceStream(assembly.GetName().Name + ".Icons." +
                                                                   name.Replace("/", ".") + ".svg"))
                        {
                            if (svg == null) return null;
                            using (var svgReader = XmlReader.Create(svg))
                            {
                                using (var w = XmlWriter.Create(s))
                                {
                                    t.Transform(svgReader, w);
                                }
                                try
                                {
                                    s.Seek(0, SeekOrigin.Begin);
                                    var sz = Encoding.UTF8.GetString(s.ToArray());

                                    using (var reader = XmlReader.Create(s))
                                        return (UIElement) System.Windows.Markup.XamlReader.Load(reader);
                                }
                                catch (IOException)
                                {
                                    return null;
                                }
                            }
                        }
                    }                    
                }
            }
        }

        public BitmapSource GetIconBitmap(string name, Size size)
        {
            var visual = GetIcon(name);

            var grid = new Grid { Width = size.Width, Height = size.Height };
            var viewbox = new Viewbox
            {
                Width = size.Width,
                Height = size.Height,
                Child = visual
            };


            grid.Children.Add(viewbox);

            grid.Measure(size);
            grid.Arrange(new Rect(size));
            //grid.UpdateLayout();

            var renderBitmap =
                new RenderTargetBitmap(
                (int)size.Width,
                (int)size.Height,
                96,
                96,
                PixelFormats.Pbgra32);
            renderBitmap.Render(grid);
            return BitmapFrame.Create(renderBitmap);
        }
    }
}
