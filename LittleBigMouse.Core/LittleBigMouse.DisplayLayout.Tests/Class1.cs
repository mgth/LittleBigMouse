using System;
using System.Collections.Generic;
using System.Drawing.Text;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Tests
{
    class Poco
    {
        public Poco(string name)
        {
            Name = name;
        }

        public string Name { get; }
    }

    class ReactivePoco : ReactiveObject
    {
        public ReactivePoco(Poco poco)
        {
            _poco = poco;
            _name = this.WhenAnyValue(e => e.Poco.Name).Log(this).ToProperty(this, e => e.Name);
        }

        public Poco Poco
        {
            get => _poco;
            set => this.RaiseAndSetIfChanged(ref _poco, value);
        }
        Poco _poco;

        public string Name => _name.Value;
        readonly ObservableAsPropertyHelper<string> _name; 
    }


    public class Class1
    {
        [Fact]
        public void Test()
        {
            var p = new ReactivePoco(new Poco("test"));

            p.Poco = new Poco("test2");

            Assert.Equal("test2", p.Name);
        }
        
    }
}
