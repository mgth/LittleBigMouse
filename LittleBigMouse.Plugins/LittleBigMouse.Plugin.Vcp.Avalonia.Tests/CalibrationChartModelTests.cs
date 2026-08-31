using HLab.Sys.Windows.MonitorVcp;
using LittleBigMouse.Plugin.Vcp.Avalonia.Calibration;
using LiveChartsCore;
using LiveChartsCore.Kernel;
using LiveChartsCore.SkiaSharpView;
using LiveChartsCore.SkiaSharpView.Painting;
using LiveChartsCore.SkiaSharpView.Painting.Effects;
using SkiaSharp;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

/// <summary>
/// Locks the projection of calibration data onto the LiveCharts series and axes.
/// These exercise the pure presentation factory extracted from the view model:
/// no UI, no hardware, no async work — just the mapping of a <see cref="Tune"/>
/// onto chart coordinates and the fixed colours/labels/limits of the axes.
/// </summary>
public class CalibrationChartModelTests
{
   static SKColor StrokeColor(ISeries series)
      => ((SolidColorPaint)((LineSeries<Tune>)series).Stroke!).Color;

   static Coordinate Map(ISeries series, Tune tune)
      => ((LineSeries<Tune>)series).Mapping!(tune, 0);

   static readonly Tune Sample = new()
   {
      Brightness = 42,
      Y = 120,
      Red = 200,
      Green = 210,
      Blue = 220,
      DeltaE = 1.5,
   };

   [Fact]
   public void BuildSeries_ProducesFiveLineSeriesInOrder()
   {
      var series = CalibrationChartModel.BuildSeries(null);

      Assert.Equal(5, series.Length);
      Assert.All(series, s => Assert.IsType<LineSeries<Tune>>(s));
   }

   [Fact]
   public void BuildSeries_UsesTheExpectedStrokeColours()
   {
      var series = CalibrationChartModel.BuildSeries(null);

      Assert.Equal(SKColors.White, StrokeColor(series[0]));
      Assert.Equal(SKColors.Red, StrokeColor(series[1]));
      Assert.Equal(SKColors.Green, StrokeColor(series[2]));
      Assert.Equal(SKColors.Blue, StrokeColor(series[3]));
      Assert.Equal(SKColors.Orange, StrokeColor(series[4]));
   }

   [Fact]
   public void BuildSeries_MapsEachChannelToBrightnessAndItsValue()
   {
      var series = CalibrationChartModel.BuildSeries(null);

      // X (secondary) is always the brightness step
      Assert.All(series, s => Assert.Equal(Sample.Brightness, Map(s, Sample).SecondaryValue));

      // Y (primary) is the channel this series charts
      Assert.Equal(Sample.Y, Map(series[0], Sample).PrimaryValue);       // luminance / white
      Assert.Equal(Sample.Red, Map(series[1], Sample).PrimaryValue);     // red gain
      Assert.Equal(Sample.Green, Map(series[2], Sample).PrimaryValue);   // green gain
      Assert.Equal(Sample.Blue, Map(series[3], Sample).PrimaryValue);    // blue gain
      Assert.Equal(Sample.DeltaE, Map(series[4], Sample).PrimaryValue);  // ΔE00 marker
   }

   [Fact]
   public void BuildSeries_AssignsSeriesToTheCorrectYAxis()
   {
      var series = CalibrationChartModel.BuildSeries(null).Cast<LineSeries<Tune>>().ToArray();

      Assert.Equal(0, series[0].ScalesYAt); // nits
      Assert.Equal(1, series[1].ScalesYAt); // gain
      Assert.Equal(1, series[2].ScalesYAt); // gain
      Assert.Equal(1, series[3].ScalesYAt); // gain
      Assert.Equal(2, series[4].ScalesYAt); // ΔE00
   }

   [Fact]
   public void BuildSeries_OnlyTheDeltaESeriesShowsAMarker()
   {
      var series = CalibrationChartModel.BuildSeries(null).Cast<LineSeries<Tune>>().ToArray();

      // the four curves are strokes only, no geometry fill
      Assert.All(series.Take(4), s =>
      {
         Assert.Null(s.GeometryFill);
         Assert.Null(s.GeometryStroke);
         Assert.Null(s.Fill);
      });

      var deltaE = series[4];
      var geometryFill = Assert.IsType<SolidColorPaint>(deltaE.GeometryFill);
      Assert.Equal(SKColors.Orange, geometryFill.Color);
      Assert.Equal(6, deltaE.GeometrySize);
      Assert.Null(deltaE.Fill);
   }

   [Fact]
   public void BuildSeries_WithNullLutLeavesValuesUnset()
   {
      var series = CalibrationChartModel.BuildSeries(null).Cast<LineSeries<Tune>>().ToArray();

      Assert.All(series, s => Assert.Null(s.Values));
   }

   [Fact]
   public void BuildXAxes_IsASingleBrightnessAxis()
   {
      var axes = CalibrationChartModel.BuildXAxes(null);

      var axis = Assert.Single(axes);
      Assert.Equal("Brightness", axis.Name);
      Assert.Equal(SKColors.Black, ((SolidColorPaint)axis.NamePaint!).Color);
      Assert.Equal(SKColors.Blue, ((SolidColorPaint)axis.LabelsPaint!).Color);
      Assert.Equal(10, axis.TextSize);
      Assert.Equal(0, axis.MinLimit);
      Assert.Null(axis.MaxLimit); // autoscales with the points
   }

   [Fact]
   public void BuildYAxes_ProducesNitsGainAndDeltaEAxes()
   {
      var axes = CalibrationChartModel.BuildYAxes(null);

      Assert.Equal(3, axes.Length);

      var nits = axes[0];
      Assert.Equal("nits", nits.Name);
      Assert.Equal(SKColors.Red, ((SolidColorPaint)nits.NamePaint!).Color);
      Assert.Equal(SKColors.Green, ((SolidColorPaint)nits.LabelsPaint!).Color);
      Assert.Equal(20, nits.TextSize);
      Assert.Equal(0, nits.MinLimit);
      Assert.Null(nits.MaxLimit);
      // dashed separators on the primary (left) axis
      var nitsSeparators = Assert.IsType<SolidColorPaint>(nits.SeparatorsPaint);
      Assert.IsType<DashEffect>(nitsSeparators.PathEffect);

      var gain = axes[1];
      Assert.Equal("Gain", gain.Name);
      Assert.Equal(SKColors.Black, ((SolidColorPaint)gain.NamePaint!).Color);
      Assert.Equal(SKColors.Blue, ((SolidColorPaint)gain.LabelsPaint!).Color);
      Assert.Equal(10, gain.TextSize);
      Assert.Equal(LiveChartsCore.Measure.AxisPosition.End, gain.Position);
      Assert.Null(gain.MinLimit); // autoscale, no fixed limits

      var deltaE = axes[2];
      Assert.Equal("ΔE00", deltaE.Name);
      Assert.Equal(SKColors.Orange, ((SolidColorPaint)deltaE.NamePaint!).Color);
      Assert.Equal(SKColors.Orange, ((SolidColorPaint)deltaE.LabelsPaint!).Color);
      Assert.Equal(10, deltaE.TextSize);
      Assert.Equal(0, deltaE.MinLimit);
      Assert.Equal(LiveChartsCore.Measure.AxisPosition.End, deltaE.Position);
      Assert.Null(deltaE.SeparatorsPaint);
   }
}
