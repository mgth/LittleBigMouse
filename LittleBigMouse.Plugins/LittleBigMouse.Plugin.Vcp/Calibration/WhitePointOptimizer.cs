namespace LittleBigMouse.Plugin.Vcp.Calibration;

/// <summary>Coordinate-descent optimizer for the monitor's RGB gain controls.</summary>
internal sealed class WhitePointOptimizer
{
    public async Task<WhitePointCalibrationResult> TuneAsync(
        ICalibrationHardware hardware,
        WhitePointCalibrationInput input,
        IProgress<CalibrationProgress>? progress,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(hardware);
        Validate(input);

        var gains = hardware.CurrentState.Gains;
        var cache = new Dictionary<CalibrationRgb, double>();
        var measurementCount = 0;
        var stablePasses = 0;
        var pass = 0;
        var channel = 0;
        var phaseCount = input.TestChannelPairs ? 6 : 3;
        var lastDeltaE = double.NaN;

        while (stablePasses < input.StablePassesRequired && pass < input.MaximumPasses)
        {
            cancellationToken.ThrowIfCancellationRequested();
            var outcome = await TuneChannelAsync(
                hardware, gains, channel, input, cache, progress,
                cancellationToken).ConfigureAwait(false);

            gains = outcome.Gains;
            if (double.IsFinite(outcome.DeltaE)) lastDeltaE = outcome.DeltaE;
            measurementCount += outcome.MeasurementCount;
            stablePasses = outcome.Changed ? 0 : stablePasses + 1;
            pass++;
            channel = (channel + 1) % phaseCount;
        }

        await hardware.SetGainsAsync(gains, cancellationToken).ConfigureAwait(false);
        var completion = stablePasses >= input.StablePassesRequired
            ? CalibrationCompletion.Converged
            : CalibrationCompletion.NotConverged;
        return new(completion, gains, lastDeltaE, measurementCount, pass);
    }

    static async Task<TuneChannelResult> TuneChannelAsync(
        ICalibrationHardware hardware,
        CalibrationRgb startingGains,
        int phase,
        WhitePointCalibrationInput input,
        Dictionary<CalibrationRgb, double> cache,
        IProgress<CalibrationProgress>? progress,
        CancellationToken cancellationToken)
    {
        var channels = phase switch
        {
            0 or 3 => new[] { 0, 1, 2 },
            1 or 4 => new[] { 1, 2, 0 },
            _ => new[] { 2, 0, 1 },
        };
        var channelCount = phase < 3 ? 1 : 2;
        var minimum = channels.Select(c => hardware.GainRanges.Channel(c).Min).ToArray();
        var maximum = channels.Select(c => input.MaximumGain == 0
            ? hardware.GainRanges.Channel(c).Max
            : Math.Min(input.MaximumGain, hardware.GainRanges.Channel(c).Max)).ToArray();
        var gains = startingGains;

        if (channelCount == 1
            && gains.Channel(channels[0]) == maximum[0]
            && gains.Channel(channels[1]) < maximum[1]
            && gains.Channel(channels[2]) < maximum[2])
            return new(gains, double.NaN, false, 0);

        if (channelCount == 2
            && (gains.Channel(channels[0]) == maximum[0]
                || gains.Channel(channels[1]) == maximum[1])
            && gains.Channel(channels[2]) < maximum[2])
            return new(gains, double.NaN, false, 0);

        var original = gains;
        var reads = 0;
        var baseline = await MeasureAtGainsAsync(hardware, gains, input.SettleDelay, cache,
            cancellationToken).ConfigureAwait(false);
        reads += baseline.WasMeasured ? 1 : 0;
        var deltaE = baseline.DeltaE;
        Report(progress, channels, channelCount, CalibrationDirection.Baseline, deltaE, deltaE);

        while (CanMove(gains, channels, channelCount, minimum, down: true))
        {
            cancellationToken.ThrowIfCancellationRequested();
            var previous = deltaE;
            gains = Move(gains, channels, channelCount, down: true);
            var measured = await MeasureAtGainsAsync(hardware, gains, input.SettleDelay, cache,
                cancellationToken).ConfigureAwait(false);
            reads += measured.WasMeasured ? 1 : 0;
            deltaE = measured.DeltaE;
            Report(progress, channels, channelCount, CalibrationDirection.Down, previous, deltaE);
            if (deltaE > previous) break;
        }

        while (CanMove(gains, channels, channelCount, maximum, down: false))
        {
            cancellationToken.ThrowIfCancellationRequested();
            var previous = deltaE;
            gains = Move(gains, channels, channelCount, down: false);
            var measured = await MeasureAtGainsAsync(hardware, gains, input.SettleDelay, cache,
                cancellationToken).ConfigureAwait(false);
            reads += measured.WasMeasured ? 1 : 0;
            deltaE = measured.DeltaE;
            Report(progress, channels, channelCount, CalibrationDirection.Up, previous, deltaE);
            if (deltaE <= previous) continue;

            var worse = deltaE;
            gains = Move(gains, channels, channelCount, down: true);
            measured = await MeasureAtGainsAsync(hardware, gains, input.SettleDelay, cache,
                cancellationToken).ConfigureAwait(false);
            reads += measured.WasMeasured ? 1 : 0;
            deltaE = measured.DeltaE;
            Report(progress, channels, channelCount, CalibrationDirection.Revert, worse, deltaE);
            break;
        }

        return new(gains, deltaE, gains != original, reads);
    }

    static async Task<CachedMeasurement> MeasureAtGainsAsync(
        ICalibrationHardware hardware,
        CalibrationRgb gains,
        TimeSpan settleDelay,
        Dictionary<CalibrationRgb, double> cache,
        CancellationToken cancellationToken)
    {
        if (cache.TryGetValue(gains, out var cached)) return new(cached, false);

        await hardware.SetGainsAsync(gains, cancellationToken).ConfigureAwait(false);
        if (settleDelay > TimeSpan.Zero)
            await Task.Delay(settleDelay, cancellationToken).ConfigureAwait(false);
        var measurement = await hardware.MeasureAsync(cancellationToken).ConfigureAwait(false);
        if (!double.IsFinite(measurement.DeltaE))
            throw new CalibrationMeasurementException("The probe returned an invalid Delta E value.");
        cache[gains] = measurement.DeltaE;
        return new(measurement.DeltaE, true);
    }

    static bool CanMove(
        CalibrationRgb gains,
        IReadOnlyList<int> channels,
        int count,
        IReadOnlyList<uint> boundary,
        bool down)
    {
        for (var i = 0; i < count; i++)
        {
            var value = gains.Channel(channels[i]);
            if (down ? value <= boundary[i] : value >= boundary[i]) return false;
        }
        return true;
    }

    static CalibrationRgb Move(
        CalibrationRgb gains,
        IReadOnlyList<int> channels,
        int count,
        bool down)
    {
        for (var i = 0; i < count; i++)
        {
            var channel = channels[i];
            var value = gains.Channel(channel);
            gains = gains.WithChannel(channel, down ? value - 1 : value + 1);
        }
        return gains;
    }

    static void Report(
        IProgress<CalibrationProgress>? progress,
        IReadOnlyList<int> channels,
        int count,
        CalibrationDirection direction,
        double previous,
        double current)
        => progress?.Report(new(CalibrationStage.AdjustingWhitePoint, Adjustment: new(
            ToChannels(channels, count), direction, previous, current)));

    static CalibrationChannels ToChannels(IReadOnlyList<int> channels, int count)
    {
        var result = CalibrationChannels.None;
        for (var i = 0; i < count; i++)
        {
            result |= channels[i] switch
            {
                0 => CalibrationChannels.Red,
                1 => CalibrationChannels.Green,
                2 => CalibrationChannels.Blue,
                _ => CalibrationChannels.None,
            };
        }
        return result;
    }

    internal static void Validate(WhitePointCalibrationInput input)
    {
        if (input.SettleDelay < TimeSpan.Zero)
            throw new ArgumentOutOfRangeException(nameof(input), "Settle delay cannot be negative.");
        if (input.StablePassesRequired <= 0)
            throw new ArgumentOutOfRangeException(nameof(input), "At least one stable pass is required.");
        if (input.MaximumPasses <= 0)
            throw new ArgumentOutOfRangeException(nameof(input), "At least one pass is required.");
    }

    readonly record struct CachedMeasurement(double DeltaE, bool WasMeasured);
    readonly record struct TuneChannelResult(
        CalibrationRgb Gains,
        double DeltaE,
        bool Changed,
        int MeasurementCount);
}
