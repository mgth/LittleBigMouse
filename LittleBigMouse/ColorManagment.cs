using System;
using System.Collections.Generic;
using System.Drawing;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace LittleBigMouse
{
    public class ProbedColor
    {
        double[] _XYZ = new double[3];
        public double[] FromXYZ(double[] XYZ)
        {
            double[,] m = D65Matrix();
            double[] rgb = new double[3];

            for (int i=0; i<3;  i++)
                rgb[i] = m[i, 0] * XYZ[0] + m[i, 1] * XYZ[1] + m[i, 2] * XYZ[2];

            return rgb;
        }

        public ProbedColor(double[] XYZ)
        {
            _XYZ = XYZ;
        }

        public double Luminance { get { return _XYZ[1]; } }

        public double getRGBComponent(uint component, double[,] matrix)
        {
            return matrix[component, 0] * _XYZ[0] + matrix[component, 1] * _XYZ[1] + matrix[component, 2] * _XYZ[2];
        }

        public double Red { get { return getRGBComponent(0, D65Matrix()); } }
        public double Green { get { return getRGBComponent(1, D65Matrix()); } }
        public double Blue { get { return getRGBComponent(2, D65Matrix()); } }
        public double NormalisedRed { get { return Red/(Red+Green+Blue); } }
        public double NormalisedGreen { get { return Green / (Red + Green + Blue); } }
        public double NormalisedBlue { get { return Blue / (Red + Green + Blue); } }

        public double DeviationRGB()
        {
            double sum = 0;
            for (uint i = 0; i < 3; i++) sum += getRGBComponent(i, D65Matrix());
            double avg = sum / 3;
            double squaresum = 0;
            for (uint i = 0; i < 3; i++) squaresum += Math.Pow(getRGBComponent(i, D65Matrix()) - avg, 2);
            double v = squaresum / 3;
            return Math.Sqrt(v);
        }
        public static double[] Normalise(double[] values)
        {
            double sum = 0;
            for (int i = 0; i < values.Length; i++) sum += values[i];
            for (int i = 0; i < values.Length; i++) values[i] /= sum;
            return values;
        }

        public static double[,] D65Matrix()
        {
            double[,] m = {
                {  3.2410, -1.5374, -0.4986 }, 
                { -0.9692,  1.8760,  0.0416 }, 
                {  0.0556, -0.2040,  1.0570 }
            };

            return m;
        }
    }

}
