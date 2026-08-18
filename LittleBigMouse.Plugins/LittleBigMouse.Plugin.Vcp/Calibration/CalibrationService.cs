namespace LittleBigMouse.Plugin.Vcp.Calibration;

/// <summary>
/// Runs calibration workflows against a small hardware port. It has no UI,
/// charting, persistence, or vendor-protocol responsibilities.
/// </summary>
public sealed class CalibrationService : ICalibrationService
{
    readonly WhitePointOptimizer _whitePoint = new();

    public Task<WhitePointCalibrationResult> TuneWhitePointAsync(
        ICalibrationHardware hardware,
        WhitePointCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default)
        => _whitePoint.TuneAsync(hardware, input, progress, cancellationToken);

    public async Task<BrightnessCalibrationResult> CalibrateBrightnessAsync(
        ICalibrationHardware hardware,
        BrightnessCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(hardware);
        WhitePointOptimizer.Validate(input.WhitePoint);

        var minimum = Math.Max(input.MinimumBrightness, hardware.BrightnessRange.Min);
        var maximum = Math.Min(input.MaximumBrightness, hardware.BrightnessRange.Max);
        var points = new List<CalibrationPoint>();
        var completion = CalibrationCompletion.Converged;
        var total = minimum <= maximum ? checked((int)(maximum - minimum + 1)) : 0;

        progress?.Report(new(CalibrationStage.Initializing, Total: total));
        for (var brightness = minimum; brightness <= maximum; brightness++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var current = checked((int)(brightness - minimum + 1));
            if (input.AlreadyMeasured.Contains(brightness))
            {
                if (brightness == uint.MaxValue) break;
                continue;
            }

            progress?.Report(new(CalibrationStage.SettingBrightness, current, total));
            await hardware.SetBrightnessAsync(brightness, cancellationToken).ConfigureAwait(false);

            var whitePoint = await _whitePoint.TuneAsync(
                hardware, input.WhitePoint, progress, cancellationToken).ConfigureAwait(false);
            if (whitePoint.Completion == CalibrationCompletion.NotConverged)
                completion = CalibrationCompletion.NotConverged;

            progress?.Report(new(CalibrationStage.Measuring, current, total));
            var measurement = await hardware.MeasureAsync(cancellationToken).ConfigureAwait(false);
            var point = new CalibrationPoint(hardware.CurrentState, measurement);
            points.Add(point);
            progress?.Report(new(CalibrationStage.MeasurementCompleted, current, total, point));

            if (brightness == uint.MaxValue) break;
        }

        progress?.Report(new(CalibrationStage.Completed, total, total));
        return new BrightnessCalibrationResult(completion, points);
    }

    public async Task<LowLuminanceCalibrationResult> CalibrateLowLuminanceAsync(
        ICalibrationHardware hardware,
        LowLuminanceCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(hardware);
        WhitePointOptimizer.Validate(input.WhitePoint);

        var original = hardware.CurrentState;
        var points = new List<CalibrationPoint>();
        var completion = CalibrationCompletion.Converged;
        var maximum = hardware.GainRanges.Red.Max;
        var minimum = hardware.GainRanges.Red.Min;
        var total = checked((int)(maximum - minimum + 1));

        try
        {
            await hardware.SetBrightnessAsync(hardware.BrightnessRange.Min, cancellationToken).ConfigureAwait(false);
            for (var gain = maximum; ; gain--)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var current = checked((int)(maximum - gain + 1));
                var whitePointInput = input.WhitePoint with { MaximumGain = gain };
                var whitePoint = await _whitePoint.TuneAsync(
                    hardware, whitePointInput, progress, cancellationToken).ConfigureAwait(false);
                if (whitePoint.Completion == CalibrationCompletion.NotConverged)
                    completion = CalibrationCompletion.NotConverged;

                progress?.Report(new(CalibrationStage.Measuring, current, total));
                var measurement = await hardware.MeasureAsync(cancellationToken).ConfigureAwait(false);
                var point = new CalibrationPoint(hardware.CurrentState, measurement);
                points.Add(point);
                progress?.Report(new(CalibrationStage.MeasurementCompleted, current, total, point));

                if (whitePoint.Gains.Min <= minimum || gain == minimum) break;
            }

            progress?.Report(new(CalibrationStage.Completed, points.Count, total));
            return new LowLuminanceCalibrationResult(completion, points);
        }
        finally
        {
            await RestoreAsync(hardware, original).ConfigureAwait(false);
        }
    }

    public async Task<ContrastCalibrationResult> ProbeContrastAsync(
        ICalibrationHardware hardware,
        ContrastCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(hardware);
        var originalGains = hardware.CurrentState.Gains;
        var minimum = Math.Max(input.MinimumContrast, hardware.ContrastRange.Min);
        var maximum = Math.Min(input.MaximumContrast, hardware.ContrastRange.Max);
        var points = new List<ContrastCalibrationPoint>();
        var totalPerChannel = minimum <= maximum ? checked((int)(maximum - minimum + 1)) : 0;
        var total = totalPerChannel * 3;

        try
        {
            for (var channel = 0; channel < 3; channel++)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var gains = new CalibrationRgb(
                    channel == 0 ? hardware.GainRanges.Red.Max : hardware.GainRanges.Red.Min,
                    channel == 1 ? hardware.GainRanges.Green.Max : hardware.GainRanges.Green.Min,
                    channel == 2 ? hardware.GainRanges.Blue.Max : hardware.GainRanges.Blue.Min);
                await hardware.SetGainsAsync(gains, cancellationToken).ConfigureAwait(false);

                for (var contrast = minimum; contrast <= maximum; contrast++)
                {
                    cancellationToken.ThrowIfCancellationRequested();
                    var current = channel * totalPerChannel + checked((int)(contrast - minimum + 1));
                    progress?.Report(new(CalibrationStage.SettingContrast, current, total));
                    await hardware.SetContrastAsync(contrast, cancellationToken).ConfigureAwait(false);
                    var measurement = await hardware.MeasureAsync(cancellationToken).ConfigureAwait(false);
                    points.Add(new(ToChannels(channel), contrast, measurement));
                    if (contrast == uint.MaxValue) break;
                }
            }

            progress?.Report(new(CalibrationStage.Completed, total, total));
            return new ContrastCalibrationResult(points);
        }
        finally
        {
            await hardware.SetGainsAsync(originalGains, CancellationToken.None).ConfigureAwait(false);
        }
    }

    static CalibrationChannels ToChannels(int channel) => channel switch
    {
        0 => CalibrationChannels.Red,
        1 => CalibrationChannels.Green,
        2 => CalibrationChannels.Blue,
        _ => CalibrationChannels.None,
    };

    static async Task RestoreAsync(
        ICalibrationHardware hardware,
        CalibrationDisplayState state)
    {
        await hardware.SetBrightnessAsync(state.Brightness, CancellationToken.None).ConfigureAwait(false);
        await hardware.SetContrastAsync(state.Contrast, CancellationToken.None).ConfigureAwait(false);
        await hardware.SetGainsAsync(state.Gains, CancellationToken.None).ConfigureAwait(false);
    }

}
