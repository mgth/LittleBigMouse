using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace NotifyChange
{
    public delegate T CalcPropertyDelegate<T>();
    public class CalcProperty<T>
    {
        private T _value;
        public bool Calculated { get; private set; }
        public bool CalculateOnChange { get; }
        private readonly CalcPropertyDelegate<T> _calc;

        public CalcProperty(CalcPropertyDelegate<T> calc, bool calculateOnChange = true)
        {
            _calc = calc;
            CalculateOnChange = calculateOnChange;
        } 

        public T Value
        {
            get
            {
                if (!Calculated)
                {
                    Calculate();
                }
                return _value;
            }
        }

        public bool Calculate()
        {
            T value = _calc();
            Calculated = true;

            if (_value.Equals(value)) return false;
            _value = value;
            return true;
        }

        public void Invalidate()
        {
            if(CalculateOnChange)
                Calculate();
            else
                Calculated = false;
        }
    }
}
