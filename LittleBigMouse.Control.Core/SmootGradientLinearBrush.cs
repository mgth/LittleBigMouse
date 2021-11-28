/*
  LittleBigMouse.Control.Core
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Reflection;
using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Imaging;

namespace LittleBigMouse.Control
{
    /// <summary>
    /// Brush that lets you draw vertical linear gradient without banding.
    /// </summary>
    public class SmoothLinearGradient
    {
        private static readonly PropertyInfo _dpiX;
        private static readonly PropertyInfo _dpiY;
        private static readonly byte[,] _bayerMatrix =
        {
        { 1, 9, 3, 11 },
        { 13, 5, 15, 7 },
        { 1, 9, 3, 11 },
        { 16, 8, 14, 6 }
    };

        static SmoothLinearGradient()
        {
            _dpiX = typeof(SystemParameters).GetProperty("DpiX", BindingFlags.NonPublic | BindingFlags.Static);
            _dpiY = typeof(SystemParameters).GetProperty("Dpi", BindingFlags.NonPublic | BindingFlags.Static);
        }

        /// <summary>
        /// Gradient color at the top
        /// </summary>
        public Color From { get; set; }

        /// <summary>
        /// Gradient color at the bottom
        /// </summary>
        public Color To { get; set; }

        public Brush GetBrush(int width, int height)
        {
            //If user changes dpi/virtual screen height during applicaiton lifetime,
            //wpf will scale the image up for us.
            int dpix = (int)_dpiX.GetValue(null);
            int dpiy = (int)_dpiY.GetValue(null);

            int stride = 4 * ((width * PixelFormats.Bgra32.BitsPerPixel + 31) / 32);

            //dithering parameters
            double bayerMatrixCoefficient = 1.0 / (_bayerMatrix.Length + 1);
            int bayerMatrixSize = _bayerMatrix.GetLength(0);

            //Create pixel data of image
            byte[] buffer = new byte[height * stride];

            for (int line = 0; line < height; line++)
            {

                for (int x = 0; x < width * 4; x += 4)
                {
                    double scale = Math.Sqrt(
                        Math.Pow((double)line / height,2)
                        *
                        Math.Pow((double)x/(width*4),2) 
                        
                        );
                    //scale = Math.Pow(scale,1/2.2);
                    //scaling of color
                    double blue = ((To.B * scale) + (From.B * (1.0 - scale)));
                    double green = ((To.G * scale) + (From.G * (1.0 - scale)));
                    double red = ((To.R * scale) + (From.R * (1.0 - scale)));
                    double alpha = ((To.A*scale) + (From.A*(1.0 - scale)));

                    //ordered dithering of color
                    //source: http://en.wikipedia.org/wiki/Ordered_dithering
                    buffer[x + line * stride] = (byte)(blue + bayerMatrixCoefficient * _bayerMatrix[x % bayerMatrixSize, line % bayerMatrixSize]);
                    buffer[x + line * stride + 1] = (byte)(green + bayerMatrixCoefficient * _bayerMatrix[x % bayerMatrixSize, line % bayerMatrixSize]);
                    buffer[x + line * stride + 2] = (byte)(red + bayerMatrixCoefficient * _bayerMatrix[x % bayerMatrixSize, line % bayerMatrixSize]);
                    buffer[x + line * stride + 3] = (byte)(alpha + bayerMatrixCoefficient * _bayerMatrix[x % bayerMatrixSize, line % bayerMatrixSize]);
                }
            }

            var image = BitmapSource.Create(width, height, dpix, dpiy, PixelFormats.Bgra32, null, buffer, stride);
            image.Freeze();
            var brush = new ImageBrush(image);
            brush.Freeze();
            return brush;
        }
    }
}
