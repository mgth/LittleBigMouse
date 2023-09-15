/*
  HLab.Argyll
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Argyll.

    HLab.Argyll is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Argyll is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;

namespace HLab.Sys.Argyll;
//http://www.brucelindbloom.com/index.html
//http://ninedegreesbelow.com/photography/xyz-rgb.html

public abstract class ProbedColor
{
    public ProbedColor Illuminant
    {
        get => _illuminant ?? this; set => _illuminant = value;
    }

    public abstract ProbedColorXYZ XYZ { get; }
    public virtual ProbedColorLab Lab => XYZ.Lab;
    public virtual ProbedColorxyY xyY => XYZ.xyY;
    public virtual ProbedColorRGB RGB(Gamut gamut) => XYZ.RGB(gamut);


    static double Calc(double T, double coef, double exp)
        => coef*Math.Pow(10, exp*3)/ Math.Pow(T, exp);

    static double Quadratic(double T, double a3, double b2, double cx, double d)
        => Calc(T, a3, 3) + Calc(T, b2, 2) + Calc(T, cx, 1) + d;

    public static ProbedColor DIlluminant(double T)
    {
        var x = (T > 7000)
            ?Quadratic(T, -2.0064, 1.9018, 0.24748, 0.237040)
            :Quadratic(T, -4.6070, 2.9678, 0.09911, 0.244063);

        var y = -3.0*x*x + 2.87*x - 0.275;

        return new ProbedColorxyY((double)x, (double)y, 1);
    }

    public ProbedColor ToGamut(Gamut gamut)
    {
        return gamut.ToGamut(this);
    }

    public ProbedColor ToLuminance(double l)
    {
        var c = xyY;
        c.Y = l;
        return c;
    }
    public ProbedColor ToSum(double v=1)
    {
        var c = XYZ;

        var sum = (c.X + c.X + c.Y)/v;

        return new ProbedColorXYZ
        {
            Illuminant = Illuminant,
            X = XYZ.X/sum,
            Y = XYZ.Y/sum,
            Z = XYZ.Z/sum
        };
    }

    public double DeltaE(ProbedColor referenceColor = null)
    {
        ProbedColorLab refLab;
        var lab = Lab;

        if (referenceColor == null)
        {
            refLab = Illuminant.Lab;
            refLab.L = lab.L;
        }
        else refLab = referenceColor.Lab;


        var result =
            (lab.L - refLab.L)*(lab.L - refLab.L)
            + (lab.a - refLab.a)*(lab.a - refLab.a)
            + (lab.b - refLab.b)*(lab.b - refLab.b);

        return Math.Sqrt(result);
    }

    public double DeltaE00(ProbedColor referenceColor = null)
    {
        ProbedColorLab refLab;
        var lab = Lab;
        //lab.L = 1;

        if (referenceColor == null)
        {
            refLab = Illuminant.Lab;
            refLab.L = lab.L;
        }
        else refLab = referenceColor.Lab;

        var Rad = Math.PI/180;
        var Rad180 = Math.PI;
        var Rad360 = 2*Rad180;

        var L1 = refLab.L;
        var a1 = refLab.a;
        var b1 = refLab.b;
        var L2 = lab.L;
        var a2 = lab.a;
        var b2 = lab.b;

        var avgLp = (L1 + L2)/2;
        var C1 = Math.Sqrt(a1*a1 + b1*b1);
        var C2 = Math.Sqrt(a2*a2 + b2*b2);

        var avgC = (C1 + C2)/2;
        var avgC7 = Math.Pow(avgC, 7);

        var G = (1 - Math.Sqrt(avgC7/(avgC7 + Math.Pow(25.0, 7.0))))/2;

        var ap1 = a1*(1 + G);
        var ap2 = a2*(1 + G);

        var Cp1 = Math.Sqrt(ap1*ap1 + b1*b1);
        var Cp2 = Math.Sqrt(ap2*ap2 + b2*b2);

        var avgCp = (Cp1 + Cp2)/2;


        var hp1 = Math.Atan2(b1, ap1);
        if (hp1 < 0) hp1 += Rad360;

        var hp2 = Math.Atan2(b2, ap2);
        if (hp2 < 0) hp2 += Rad360;

        var avgHp = (hp1 + hp2)/2;
        if (Math.Abs(hp1 - hp2) > Rad180) avgHp += Rad180;

        var T = 1
                - 0.17*Math.Cos(1*avgHp - 30.0*Rad)
                + 0.24*Math.Cos(2*avgHp)
                + 0.32*Math.Cos(3*avgHp + 6.0*Rad)
                - 0.20*Math.Cos(4*avgHp - 63.0*Rad);

        var dhp = hp2 - hp1;
        if (Math.Abs(dhp) > Rad180)
        {
            if (hp2 > hp1)
                dhp -= Rad360;
            else
                dhp += Rad360;
        }

        var dLp = L2 - L1;
        var dCp = Cp2 - Cp1;

        var dHp = 2*Math.Sqrt(Cp1*Cp2)*Math.Sin(dhp/2);

        var avgLp50 = Math.Pow(avgLp - 50.0, 2);

        var Sl = 1 + (0.015*avgLp50/Math.Sqrt(20.0 + avgLp50));
        var Sc = 1 + 0.045*avgCp;
        var Sh = 1 + 0.015*avgCp*T;

        var dO = (30.0*Rad)*Math.Exp(-Math.Pow(avgHp - 275.0*Rad, 2));

        var avgCp7 = Math.Pow(avgCp, 7);

        var Rc = 2*Math.Sqrt(avgCp7/(avgCp7 + Math.Pow(25.0, 7.0)));

        var Rt = -Rc*Math.Sin(2*dO);

        const double Kl = 1.0;
        const double Kc = 1.0;
        const double Kh = 1.0;



        var de =
            Math.Pow(dLp/(Kl*Sl), 2)
            + Math.Pow(dCp/(Kc*Sc), 2)
            + Math.Pow(dHp/(Kh*Sh), 2)
            + Rt*(dCp/(Kc*Sc))*(dHp/Kh*Sh);

        return Math.Sqrt(de);
    }

    private readonly double[] _rt =
    {
        /* reciprocal temperature (K) */
        double.MinValue, 10.0e-6, 20.0e-6, 30.0e-6, 40.0e-6, 50.0e-6,
        60.0e-6, 70.0e-6, 80.0e-6, 90.0e-6, 100.0e-6, 125.0e-6,
        150.0e-6, 175.0e-6, 200.0e-6, 225.0e-6, 250.0e-6, 275.0e-6,
        300.0e-6, 325.0e-6, 350.0e-6, 375.0e-6, 400.0e-6, 425.0e-6,
        450.0e-6, 475.0e-6, 500.0e-6, 525.0e-6, 550.0e-6, 575.0e-6,
        600.0e-6
    };

    class UVT
    {
        public readonly double U;
        public readonly double V;
        public readonly double T;

        public UVT(double u, double v, double t)
        {
            U = u;
            V = v;
            T = t;
        }
    }

    private readonly UVT[] _uvt =
    {
        new UVT(0.18006, 0.26352, -0.24341),
        new UVT(0.18066, 0.26589, -0.25479),
        new UVT(0.18133, 0.26846, -0.26876),
        new UVT(0.18208, 0.27119, -0.28539),
        new UVT(0.18293, 0.27407, -0.30470),
        new UVT(0.18388, 0.27709, -0.32675),
        new UVT(0.18494, 0.28021, -0.35156),
        new UVT(0.18611, 0.28342, -0.37915),
        new UVT(0.18740, 0.28668, -0.40955),
        new UVT(0.18880, 0.28997, -0.44278),
        new UVT(0.19032, 0.29326, -0.47888),
        new UVT(0.19462, 0.30141, -0.58204),
        new UVT(0.19962, 0.30921, -0.70471),
        new UVT(0.20525, 0.31647, -0.84901),
        new UVT(0.21142, 0.32312, -1.0182),
        new UVT(0.21807, 0.32909, -1.2168),
        new UVT(0.22511, 0.33439, -1.4512),
        new UVT(0.23247, 0.33904, -1.7298),
        new UVT(0.24010, 0.34308, -2.0637),
        new UVT(0.24792, 0.34655, -2.4681),
        /* Note: 0.24792 is a corrected value for the error found in W&S as 0.24702 */
        new UVT(0.25591, 0.34951, -2.9641),
        new UVT(0.26400, 0.35200, -3.5814),
        new UVT(0.27218, 0.35407, -4.3633),
        new UVT(0.28039, 0.35577, -5.3762),
        new UVT(0.28863, 0.35714, -6.7262),
        new UVT(0.29685, 0.35823, -8.5955),
        new UVT(0.30505, 0.35907, -11.324),
        new UVT(0.31320, 0.35968, -15.628),
        new UVT(0.32129, 0.36011, -23.325),
        new UVT(0.32931, 0.36038, -40.770),
        new UVT(0.33724, 0.36051, -116.45)
    };

    private ProbedColor _illuminant;


    static double Lerp(double a, double b, double c) => (b - a)*c + a;

    public double ColorTemp
    {
        get
        {


            var xyz = XYZ;

            if ((xyz.X < 1.0e-20) && (xyz.Y < 1.0e-20) && (xyz.Z < 1.0e-20))
                return (-1); /* protect against possible divide-by-zero failure */
            var us = (4.0*xyz.X)/(xyz.X + 15.0*xyz.Y + 3.0*xyz.Z);
            var vs = (6.0*xyz.Y)/(xyz.X + 15.0*xyz.X + 3.0*xyz.Z);
            var dm = 0.0;
            var di = dm;

            int i;
            for (i = 0; i < 31; i++)
            {
                di = (vs - _uvt[i].V) - _uvt[i].T*(us - _uvt[i].U);
                if ((i > 0) && (((di < 0.0) && (dm >= 0.0)) || ((di >= 0.0) && (dm < 0.0))))
                    break; /* found lines bounding (us, vs) : i-1 and i */
                dm = di;
            }
            if (i == 31)
                return (-1);
            /* bad XYZ input, color temp would be less than minimum of 1666.7 degrees, or too far towards blue */
            di = di/Math.Sqrt(1.0 + _uvt[i].T*_uvt[i].T);
            dm = dm/Math.Sqrt(1.0 + _uvt[i - 1].T*_uvt[i - 1].T);
            var p = dm/(dm - di);
            p = 1.0/(Lerp(_rt[i - 1], _rt[i], p));
            return p;
        }
    }

}

public class ProbedColorXYZ : ProbedColor
{
    public double X { get; set; }
    public double Y { get; set; }
    public double Z { get; set; }
    public override ProbedColorXYZ XYZ => this;

    public double Max => Math.Max(Math.Max(X,Y),Z);
    public double Min => Math.Min(Math.Min(X, Y), Z);

    public ProbedColorXYZ Mult(double ratio) => new ProbedColorXYZ
    {
        X = X * ratio,
        Y = Y * ratio,
        Z = Z * ratio,
        Illuminant = Illuminant
    };



    public ProbedColorXYZ Maximised
    {
        get
        {

            return new ProbedColorXYZ
            {
                X = X / Max,
                Y = Y / Max,
                Z = Z / Max,
            };

            var xyz = new ProbedColorXYZ {X = X, Y = Y, Z = Z, Illuminant = Illuminant};
            if (xyz.X > 1)
            {
                xyz.Y /= xyz.X;
                xyz.Z /= xyz.X;
                xyz.X = 1.0;
            }
            if (xyz.Y > 1)
            {
                xyz.X /= xyz.Y;
                xyz.Z /= xyz.Y;
                xyz.Y = 1.0;
            }
            if (xyz.Z > 1)
            {
                xyz.X /= xyz.Z;
                xyz.Y /= xyz.Z;
                xyz.Z = 1.0;
            }
            return xyz;
        }
    }

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
            var white = Illuminant.XYZ;

            var fX = LabF(X / white.X);
            var fY = LabF(Y / white.Y);
            var fZ = LabF(Z / white.Z);

            var lab = new ProbedColorLab
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
            var sum = X + Y + Z;

            if (sum == 0) return new ProbedColorxyY(0,0,0) {Illuminant = Illuminant};


            var xyy = new ProbedColorxyY(
                X / sum, 
                Y / sum, 
                Y)
            {
                Illuminant = Illuminant,
            };
            return xyy;
        }
    }


    public Matrix2D Matrix => new Matrix2D( new double[,]
    {
        { X },
        { Y },
        { Z }
    });

    public override ProbedColorRGB RGB(Gamut gamut) 
        => new ProbedColorRGB(
            Illuminant,
            gamut.XYZ_RGB_Matrix * ToGamut(gamut).XYZ.Matrix
        );
        
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

            var fy = (L + 16)/116;
            var fz = fy - (b/200);
            var fx = (a/500) + fy;

            var fx3 = Math.Pow(fx, 3);
            var fz3 = Math.Pow(fz, 3);
            var x = (fx3 > e) ? fx3 : (116*fx - 16)/k;
            var y = (L > k*e) ? Math.Pow((L + 16)/116, 3) : L/k;
            var z = (fz3 > e) ? fz3 : (116*fz - 16)/k;

            var white = Illuminant.XYZ;

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
    public ProbedColorxyY(double x, double y, double Y)
    {
        this.x = x;
        this.y = y;
        this.Y = Y;
    }

    public ProbedColorxyY()
    {
    }

    public double x { get; set; }
    public double y { get; set; }
    public double Y { get; set; }

    public override ProbedColorxyY xyY => this;

    public override ProbedColorXYZ XYZ
    {
        get
        {
            ProbedColorXYZ XYZ;

            if (y == 0) y = 1/10000;
            {
                XYZ = new ProbedColorXYZ
                {
                    Illuminant = Illuminant,
                    X = (x*Y)/y,
                    Y = Y,
                    Z = ((1 - x - y)*Y)/y,
                };
            }

            //else XYZ = new ProbedColorXYZ
            //{
            //    Illuminant = Illuminant,
            //    X=x*Y, Y=Y, Z=(1 - x - y) * Y
            //};

            return XYZ;
        }
    }
}
public class ProbedColorRGB : ProbedColor
{
    public double R { get; set; }
    public double G { get; set; }
    public double B { get; set; }
    public override ProbedColorXYZ XYZ { get; }

    public ProbedColorRGB(ProbedColor illuminant, Matrix2D matrix)
    {
        Illuminant = illuminant;
        R = matrix[0, 0];
        G = matrix[1, 0];
        B = matrix[2, 0];
    }

    public ProbedColorRGB(ProbedColor illuminant, double r, double g, double b)
    {
        Illuminant = illuminant;
        R = r;
        G = g;
        B = b;
    }

    public ProbedColorRGB Normalized
    {
        get
        {
            var max = Math.Max(Math.Max(R, G),B);

            var RGB = new ProbedColorRGB  ( Illuminant, R/max, G/max, B/max ) ;

            return RGB;
        }
    }

    public ProbedColorRGB Saturated
    {
        get
        {
            var min = Math.Min(Math.Min(R, G), B);
            var max = Math.Max(Math.Max(R, G), B) - min;

            var RGB = new ProbedColorRGB(Illuminant, (R - min)/max, (G - min)/max, (B - min)/max);

            return RGB;
        }
    }


    public ProbedColorRGB Bits(int n)
    {
        var max = Math.Pow(2, n) - 1;

        return new ProbedColorRGB(
            Illuminant,
            Math.Min(R * max, max),
            Math.Min(G * max, max),
            Math.Min(B * max, max)
        );
    }

    public ProbedColorRGB sRGBCompanding => new ProbedColorRGB(

        Illuminant,
        Companding(R),
        Companding(G),
        Companding(B)
    );
    private static double Companding(double v)
    {
        if (v > 0.0031308) return 1.055 * Math.Pow(v, 1.0 / 2.4) - 0.055;
        return 12.92 * v;
    }

    public ProbedColorRGB GammaCompanding(double gamma) => new ProbedColorRGB(

        Illuminant,
        GammaComp(R,gamma),
        GammaComp(G,gamma),
        GammaComp(B,gamma)
    );
    private static double GammaComp(double v,double gamma)
    {
        return Math.Pow(v, 1/gamma);
    }

    public ProbedColorRGB LCompanding => new ProbedColorRGB(

        Illuminant,
        LComp(R),
        LComp(G),
        LComp(B)
    );
    private static double LComp(double v)
    {
        var k = 24389.0/27.0;
        var e = 216.0/24389.0;
        return (v > e) ? 1.16*Math.Pow(v, 1.0/3.0) - 0.16:v*k/100.0;
    }

    public (byte,byte,byte) Color
    {
        get
        {
            var rgb = Bits(8);
            return ((byte)rgb.R, (byte)rgb.G, (byte)rgb.B);
        }
    }
}