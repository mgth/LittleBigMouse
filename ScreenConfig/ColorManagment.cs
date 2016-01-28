using System;
using System.Runtime.CompilerServices;

namespace LbmScreenConfig
{
    //http://www.brucelindbloom.com/index.html

 
    



    public abstract class ProbedColor
    {
        public ProbedColor Illuminant { get; set; }
        public abstract ProbedColorXYZ XYZ { get; }
        public virtual ProbedColorLab Lab => XYZ.Lab;
        public virtual ProbedColorxyY xyY => XYZ.xyY;
        public virtual ProbedColorRGB RGB => XYZ.RGB;


        private static double Calc(double T, double coef, double exp)
            => coef*Math.Pow(10, exp * 3)/Math.Pow(T, exp);

        private static double Quadratic(double T, double a3, double b2, double cx, double d)
            => Calc(T, a3, 3) + Calc(T, b2, 2) + Calc(T, cx, 1) + d;

        public static ProbedColor DIlluminant(double T)
        {
            ProbedColorxyY xyY = new ProbedColorxyY();
            if (T > 7000)
            {
                xyY.x = Quadratic(T, -2.0064, 1.9018, 0.24748, 0.237040);
            }
            else
            {
                xyY.x = Quadratic(T, -4.6070, 2.9678, 0.09911, 0.244063);            
            }

            xyY.y = -3.0 * Math.Pow(xyY.x, 2) + 2.87 * xyY.x - 0.275;

            xyY.Y = 1;

            xyY.Illuminant = xyY;

            return xyY;
        }

        public double DeltaE(ProbedColor referenceColor = null)
        {
            ProbedColorLab refLab;
            ProbedColorLab lab = Lab;

            if (referenceColor == null)
            {
                refLab = Illuminant.Lab;
                refLab.L = lab.L;
            }
            else refLab = referenceColor.Lab;


            double result =
                (lab.L - refLab.L)*(lab.L - refLab.L)
                + (lab.a - refLab.a)*(lab.a - refLab.a)
                + (lab.b - refLab.b)*(lab.b - refLab.b);

            return Math.Sqrt(result);
        }

        public double DeltaE00(ProbedColor referenceColor = null)
        {
            ProbedColorLab refLab;
            ProbedColorLab lab = Lab;
            //lab.L = 1;

            if (referenceColor == null)
            {
                refLab = Illuminant.Lab;
                refLab.L = lab.L;
            }
            else refLab = referenceColor.Lab;

            double Rad = Math.PI/180;
            double Rad180 = Math.PI;
            double Rad360 = 2*Rad180;

            double L1 = refLab.L;
            double a1 = refLab.a;
            double b1 = refLab.b;
            double L2 = lab.L;
            double a2 = lab.a;
            double b2 = lab.b;

            double avgLp = (L1 + L2)/2;
            double C1 = Math.Sqrt(a1 * a1 + b1 * b1);
            double C2 =  Math.Sqrt(a2 * a2 + b2 * b2);

            double avgC = (C1 + C2)/2;
            double avgC7 = Math.Pow(avgC, 7);

            double G = (1 - Math.Sqrt(avgC7/(avgC7 + Math.Pow(25.0, 7.0))))/2;

            double ap1 = a1*(1 + G);
            double ap2 = a2*(1 + G);

            double Cp1 = Math.Sqrt(ap1*ap1 + b1*b1);
            double Cp2 = Math.Sqrt(ap2*ap2 + b2*b2);

            double avgCp = (Cp1 + Cp2)/2;


            double hp1 = Math.Atan2(b1, ap1);
            if (hp1 < 0) hp1 += Rad360;

            double hp2 = Math.Atan2(b2, ap2);
            if (hp2 < 0) hp2 += Rad360;

            double avgHp = (hp1 + hp2)/2;
            if (Math.Abs(hp1 - hp2) > Rad180) avgHp += Rad180;

            double T = 1
                       - 0.17 * Math.Cos(1*avgHp - 30.0 * Rad)
                       + 0.24 * Math.Cos(2*avgHp)
                       + 0.32 * Math.Cos(3*avgHp + 6.0 * Rad)
                       - 0.20 * Math.Cos(4*avgHp - 63.0 * Rad);

            double dhp = hp2 - hp1;
            if (Math.Abs(dhp) > Rad180)
            {
                if (hp2 > hp1)
                    dhp -= Rad360;
                else
                    dhp += Rad360;           
            }

            double dLp = L2 - L1;
            double dCp = Cp2 - Cp1;

            double dHp = 2*Math.Sqrt(Cp1*Cp2)*Math.Sin(dhp/2);

            double avgLp50 = Math.Pow(avgLp - 50.0, 2);

            double Sl = 1 + (0.015 * avgLp50 / Math.Sqrt(20.0 + avgLp50));
            double Sc = 1 + 0.045 * avgCp;
            double Sh = 1 + 0.015 * avgCp * T;

            double dO = (30.0 * Rad) *Math.Exp(-Math.Pow(avgHp - 275.0 * Rad,2));

            double avgCp7 = Math.Pow(avgCp, 7);

            double Rc = 2*Math.Sqrt(avgCp7/(avgCp7 + Math.Pow(25.0, 7.0)));

            double Rt = -Rc*Math.Sin(2*dO);

            const double Kl = 1.0;
            const double Kc = 1.0;
            const double Kh = 1.0;



            double de =
                Math.Pow(dLp/(Kl*Sl), 2)
                + Math.Pow(dCp/(Kc*Sc), 2)
                + Math.Pow(dHp/(Kh*Sh), 2)
                + Rt*(dCp/(Kc*Sc))*(dHp/Kh*Sh);

            return Math.Sqrt(de);
        }

        public double[,] RgbMatrix => new double[,] {};
    }

    public class ProbedColorXYZ : ProbedColor
    {
        public double X { get; set; }
        public double Y { get; set; }
        public double Z { get; set; }
        public override ProbedColorXYZ XYZ => this;
        private static double LabF(double t)
        {
            if (t > Math.Pow(6.0 / 29.0, 3)) return Math.Pow(t, 1.0 / 3.0);
            else
            {
                return (1.0 / 3.0) * (29.0 / 6.0) * (29.0 / 6.0) * t + (4.0 / 29.0);
            }
        }

        public override ProbedColorLab Lab
        {
            get
            {
                ProbedColorXYZ white = Illuminant.XYZ;

                double fX = LabF(X / white.X);
                double fY = LabF(Y / white.Y);
                double fZ = LabF(Z / white.Z);

                ProbedColorLab lab = new ProbedColorLab
                {
                    Illuminant = Illuminant,
                    L = 116*fY - 16,
                    a = 500*(fX - fY),
                    b = 200*(fY - fZ),
                };

                return lab;
            }
        }

        public override ProbedColorxyY xyY
        {
            get
            {
                ProbedColorxyY xyy = new ProbedColorxyY
                {
                    Illuminant = Illuminant,
                    x = X / (X+Y+Z),
                    y = Y / (X+Y+Z),
                    Y = Y
                };
                return xyy;
            }
        }

        public override ProbedColorRGB RGB
        {
            get 
            {
                double[,] m = Illuminant.RgbMatrix;
                ProbedColorRGB rgb = new ProbedColorRGB
                {
                    Illuminant = Illuminant,
                    R = m[0, 0]*X + m[0, 1]*Y + m[0, 2]*Z,
                    G = m[1, 0]*X + m[1, 1]*Y + m[1, 2]*Z,
                    B = m[2, 0]*X + m[2, 1]*Y + m[2, 2]*Z
                };
                return rgb;
            }
        }
    }
    public class ProbedColorLab : ProbedColor
    {
        public double L { get; set; }
        public double a { get; set; }
        public double b { get; set; }

        public override ProbedColorXYZ XYZ
        {
            get
            {
                const double e = 216.0/24389.0;
                const double k = 24389.0/27.0;

                double fy = (L + 16)/116;
                double fz = fy - (b/200);
                double fx = (a/500) + fy;

                double fx3 = Math.Pow(fx, 3);
                double fz3 = Math.Pow(fz, 3);
                double x = (fx3 > e) ? fx3 : (116*fx - 16)/k;
                double y = (L > k*e) ? Math.Pow((L + 16)/116, 3) : L/k;
                double z = (fz3 > e) ? fz3 : (116*fz - 16)/k;

                ProbedColorXYZ white = Illuminant.XYZ;

                return new ProbedColorXYZ
                {
                    Illuminant = Illuminant,
                    X = x*white.X,
                    Y = y*white.Y,
                    Z = z*white.Z
                };
            }
        }
    }
    public class ProbedColorxyY : ProbedColor
    {
        public double x { get; set; }
        public double y { get; set; }
        public double Y { get; set; }

        public override ProbedColorxyY xyY => this;

        public override ProbedColorXYZ XYZ
        {
            get
            {
                if (y > 0)
                {
                    return new ProbedColorXYZ
                    {
                        Illuminant = Illuminant,
                        X = (x*Y)/y,
                        Y = Y,
                        Z = (1 - x - y)*Y/y,
                    };
                }

                return new ProbedColorXYZ
                {
                    Illuminant = Illuminant,
                    X=0, Y=0, Z=0
                };
            }
        }
    }
    public class ProbedColorRGB : ProbedColor
    {
        public double R { get; set; }
        public double G { get; set; }
        public double B { get; set; }
        public override ProbedColorXYZ XYZ { get; }
        public override ProbedColorRGB RGB => this;
    }



}