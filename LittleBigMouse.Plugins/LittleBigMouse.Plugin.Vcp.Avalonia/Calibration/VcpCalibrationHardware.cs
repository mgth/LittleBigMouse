using HLab.Sys.Argyll;
using HLab.Sys.Windows.MonitorVcp;
using LittleBigMouse.Plugin.Vcp.Calibration;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Calibration;

/// <summary>
/// Adapts the existing DDC/CI controls and blocking Argyll probe to the
/// UI-neutral calibration hardware port.
/// </summary>
internal sealed class VcpCalibrationHardware(VcpControl control, ArgyllProbe probe)
    : ICalibrationHardware
{
    public CalibrationRange BrightnessRange => control.Brightness is { } level
        ? new(level.Min, level.Max)
        : new(0, 0);

    public CalibrationRange ContrastRange => control.Contrast is { } level
        ? new(level.Min, level.Max)
        : new(0, 0);

    public CalibrationRgbRanges GainRanges => control.Gain is { } gain
        ? new(
            new(gain.Red.Min, gain.Red.Max),
            new(gain.Green.Min, gain.Green.Max),
            new(gain.Blue.Min, gain.Blue.Max))
        : new(new(0, 0), new(0, 0), new(0, 0));

    public CalibrationDisplayState CurrentState
    {
        get
        {
            var gains = control.Gain?.GetValues() ?? [0, 0, 0];
            return new(
                control.Brightness?.Value ?? 0,
                control.Contrast?.Value ?? 0,
                new(gains[0], gains[1], gains[2]));
        }
    }

    public Task SetBrightnessAsync(uint value, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (control.Brightness is { } level) level.Value = ClampLevel(level, value);
        return Task.CompletedTask;
    }

    public Task SetContrastAsync(uint value, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (control.Contrast is { } level) level.Value = ClampLevel(level, value);
        return Task.CompletedTask;
    }

    public Task SetGainsAsync(CalibrationRgb gains, CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        control.Gain?.SetTo([gains.Red, gains.Green, gains.Blue]);
        return Task.CompletedTask;
    }

    public async Task<CalibrationMeasurement> MeasureAsync(CancellationToken cancellationToken)
    {
        cancellationToken.ThrowIfCancellationRequested();
        using var registration = cancellationToken.Register(probe.Abort);
        var succeeded = await Task.Run(probe.SpotRead).ConfigureAwait(false);
        cancellationToken.ThrowIfCancellationRequested();
        if (!succeeded)
            throw new CalibrationMeasurementException("ArgyllCMS could not obtain a measurement.");

        var color = probe.ProbedColor;
        return new(
            color.xyY.Y,
            color.xyY.x,
            color.xyY.y,
            color.DeltaE00());
    }

    static uint ClampLevel(MonitorLevel level, uint value) => Math.Clamp(value, level.Min, level.Max);
}
