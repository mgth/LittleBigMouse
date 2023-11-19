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
using HLab.Geo;

namespace HLab.Sys.Argyll;

public class Gamut
{
    public ProbedColor RedPrimary { get; }
    public ProbedColor GreenPrimary { get; }
    public ProbedColor BluePrimary { get; }
    public ProbedColor WhitePrimary { get; }

    public Gamut(ProbedColor red, ProbedColor green, ProbedColor blue, ProbedColor white)
    {
        RedPrimary = red;
        GreenPrimary = green;
        BluePrimary = blue;
        WhitePrimary = white;
    }

    public ProbedColor ToGamut(ProbedColor c)
    {
        Triangle t = new Triangle(
            new Point(RedPrimary.xyY.x,RedPrimary.xyY.y),
            new Point(GreenPrimary.xyY.x, GreenPrimary.xyY.y),
            new Point(BluePrimary.xyY.x, BluePrimary.xyY.y)
        );

        Point p = t.Inside(new Point(WhitePrimary.xyY.x, WhitePrimary.xyY.y), new Point(c.xyY.x, c.xyY.y));

        return new ProbedColorxyY(p.X,p.Y,c.xyY.Y);
    }

    public Matrix2D RGB_XYZ_Matrix
    {
        get
        {
            ProbedColorXYZ r = RedPrimary.ToLuminance(1).XYZ;
            ProbedColorXYZ g = GreenPrimary.ToLuminance(1).XYZ;
            ProbedColorXYZ b = BluePrimary.ToLuminance(1).XYZ;
            ProbedColorXYZ w = WhitePrimary.ToLuminance(1).XYZ;

            Matrix2D M1 = new Matrix2D(new double[,]{
                {r.X,g.X,b.X},
                {r.Y,g.Y,b.Y},
                {r.Z,g.Z,b.Z},
            });
                

            Matrix2D S = M1.Inverse * w.Matrix;

            double Sr = S[0, 0];
            double Sg = S[1, 0];
            double Sb = S[2, 0];

            Matrix2D M = new Matrix2D(new double[,]
            {
                {Sr*r.X, Sg*g.X, Sb*b.X },
                {Sr*r.Y, Sg*g.Y, Sb*b.Y },
                {Sr*r.Z, Sg*g.Z, Sb*b.Z }
            });

            return M;
        }
    }

    public Matrix2D XYZ_RGB_Matrix => RGB_XYZ_Matrix.Inverse;
}

public static class Gamuts
{
    public static Gamut ProPhotoRGB => new Gamut(
        new ProbedColorxyY(0.7347, 0.2653, 0.28804),
        new ProbedColorxyY(0.1596, 0.8404, 0.711874),
        new ProbedColorxyY(0.0366, 0.0001, 0.000086),
        ProbedColor.DIlluminant(5000)
    );
    public static Gamut sRGB => new Gamut(
        new ProbedColorxyY(0.6400,   0.3300,  0.212656),
        new ProbedColorxyY(0.3000,   0.6000,  0.715158),
        new ProbedColorxyY(0.1500,   0.0600,  0.072186),
        ProbedColor.DIlluminant(6500)
    );

    public static Gamut AdobeRGB98 => new Gamut(
        new ProbedColorxyY(0.6400,   0.3300,  0.297361),
        new ProbedColorxyY(0.2100,   0.7100,  0.627355),
        new ProbedColorxyY(0.1500,   0.0600,  0.075285),
        ProbedColor.DIlluminant(6500)
    );

}
//		