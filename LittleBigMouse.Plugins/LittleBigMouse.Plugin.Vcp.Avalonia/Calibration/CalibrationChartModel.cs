/*
  LittleBigMouse.Plugin.Vcp.Avalonia
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Vcp.Avalonia.

    LittleBigMouse.Plugin.Vcp.Avalonia is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Vcp.Avalonia is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using HLab.Sys.Windows.MonitorVcp;
using LiveChartsCore;
using LiveChartsCore.Kernel;
using LiveChartsCore.SkiaSharpView;
using LiveChartsCore.SkiaSharpView.Painting;
using LiveChartsCore.SkiaSharpView.Painting.Effects;
using SkiaSharp;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Calibration;

/// <summary>
/// Pure presentation factory turning a <see cref="ProbeLut"/> into the LiveCharts
/// series and axes used by the calibration curve. It knows nothing about the
/// calibration algorithm, the DDC/CI hardware or the smart-TV clients, and never
/// starts asynchronous work: given a lut, it produces the exact same series,
/// colours, labels and axis settings the view model used to build inline.
/// </summary>
public static class CalibrationChartModel
{
   /// <summary>
   /// Builds the calibration curve series for <paramref name="lut"/>.
   /// The white/red/green/blue lines follow the smoothed lut; the ΔE00 marker
   /// series follows the raw sorted lut. A null lut yields empty (null-valued)
   /// series, exactly as the previous inline construction did.
   /// </summary>
   public static ISeries[] BuildSeries(ProbeLut? lut) =>
   [
      new LineSeries<Tune>
      {
            Values = lut?.SmoothLut,
            Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Y),

            Stroke = new SolidColorPaint(SKColors.White),
            GeometryStroke = null,
            GeometryFill = null,
            Fill = null,

      },

      new LineSeries<Tune>
      {
            Values = lut?.SmoothLut,
            Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Red),

            Stroke = new SolidColorPaint(SKColors.Red),
            GeometryStroke = null,
            GeometryFill = null,
            Fill = null,
            ScalesYAt = 1
      },
      new LineSeries<Tune>
      {
            Values = lut?.SmoothLut,
            Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Green),

            Stroke = new SolidColorPaint(SKColors.Green),
            GeometryStroke = null,
            GeometryFill = null,
            Fill = null,
            ScalesYAt = 1
      },
      new LineSeries<Tune>
      {
            Values = lut?.SmoothLut,
            Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.Blue),

            Stroke = new SolidColorPaint(SKColors.Blue),
            GeometryStroke = null,
            GeometryFill = null,
            Fill = null,
            ScalesYAt = 1
      },

      // measured ΔE00 after tuning, one point per brightness step
      new LineSeries<Tune>
      {
            Values = lut?.SortedLut,
            Mapping = (tune, index) => new Coordinate(tune.Brightness, tune.DeltaE),

            Stroke = new SolidColorPaint(SKColors.Orange),
            GeometryStroke = null,
            GeometryFill = new SolidColorPaint(SKColors.Orange),
            GeometrySize = 6,
            Fill = null,
            ScalesYAt = 2
      },

   ];

   /// <summary>
   /// Builds the single horizontal (brightness) axis. The <paramref name="lut"/>
   /// argument is kept for parity with the previous binding, which recomputed the
   /// axes whenever the lut changed; the axis itself autoscales to the points.
   /// </summary>
   public static Axis[] BuildXAxes(ProbeLut? lut) =>
   [
      new Axis
      {
         Name = "Brightness",
         NamePaint = new SolidColorPaint(SKColors.Black),

         LabelsPaint = new SolidColorPaint(SKColors.Blue),
         TextSize = 10,

         SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray) { StrokeThickness = 2 }  ,
         // no MaxLimit: autoscale follows the points as they land
         MinLimit = 0,

      },
   ];

   /// <summary>
   /// Builds the three vertical axes: nits (left), gain (right), ΔE00 (right).
   /// </summary>
   public static Axis[] BuildYAxes(ProbeLut? lut) =>
   [
      new Axis
      {
         Name = "nits",
         NamePaint = new SolidColorPaint(SKColors.Red),

         LabelsPaint = new SolidColorPaint(SKColors.Green),
         TextSize = 20,

         SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray)
         {
            StrokeThickness = 2,
            PathEffect = new DashEffect([3, 3])
         }            ,
         // no MaxLimit: autoscale follows the points as they land
         MinLimit = 0,


      },

      new Axis
      {
         Name = "Gain",
         NamePaint = new SolidColorPaint(SKColors.Black),

         LabelsPaint = new SolidColorPaint(SKColors.Blue),
         TextSize = 10,

         SeparatorsPaint = new SolidColorPaint(SKColors.LightSlateGray) { StrokeThickness = 2 }  ,
         // autoscale: fixed limits computed at bind time went stale
         // (and threw on an empty lut) once live measurements landed
         Position = LiveChartsCore.Measure.AxisPosition.End
      },

      new Axis
      {
         Name = "ΔE00",
         NamePaint = new SolidColorPaint(SKColors.Orange),

         LabelsPaint = new SolidColorPaint(SKColors.Orange),
         TextSize = 10,

         SeparatorsPaint = null,
         MinLimit = 0,
         Position = LiveChartsCore.Measure.AxisPosition.End
      }

   ];
}
