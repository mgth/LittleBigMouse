using BenchmarkDotNet.Attributes;

namespace BenchNetFramework
{
    interface IBaseInterface
    {
    }

    interface ITestInterface
    {
        void Dotest();
    }

    class TestClass : IBaseInterface, ITestInterface
    {
        private int i = 0;
        public void Dotest()
        {
            i++;
        }
    }
    abstract class TestClassVirtual : IBaseInterface, ITestInterface
    {
        private int i = 0;
        public abstract void Dotest();
    }

    class SubClass : TestClassVirtual
    {
        private int i = 0;
        public override void Dotest()
        {
            i++;
        }
    }

    class TestClassExplicite : IBaseInterface, ITestInterface
    {
        private int i = 0;
        void ITestInterface.Dotest()
        {
            i++;
        }
    }

    public class BenchCasts
    {
        private readonly TestClass _typed = new TestClass();
        private readonly TestClassVirtual _subclass = new SubClass();
        private readonly SubClass _subclassDirect = new SubClass();
        private readonly object _nontyped = new TestClass();
        private readonly ITestInterface _iface = new TestClass();
        private readonly ITestInterface _ifaceExplicite = new TestClassExplicite();

        [Benchmark(Baseline = true)]
        public void NoCast()
        {
            _typed.Dotest();
        }

        [Benchmark]
        public void SubClass()
        {
            _subclass.Dotest();
        }

        [Benchmark]
        public void SubClassDirect()
        {
            _subclassDirect.Dotest();
        }

        [Benchmark]
        public void Interface()
        {
            _iface.Dotest();
        }
        [Benchmark]
        public void InterfaceExplicite()
        {
            _ifaceExplicite.Dotest();
        }

        [Benchmark]
        public void DirectCastClass()
        {
            ((TestClass)_nontyped).Dotest();
        }

        [Benchmark]
        public void DirectCastInterface()
        {
            ((ITestInterface)_nontyped).Dotest();
        }

        [Benchmark]
        public void BenchIs()
        {
            if(_nontyped is ITestInterface t) t.Dotest();
        }

    }
}
