using LittleBigMouse.Plugin.Vcp.Calibration;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class CalibrationServiceTests
{
    readonly CalibrationService _service = new();

    [Fact]
    public async Task BrightnessCalibrationReturnsMeasuredBusinessPoints()
    {
        var hardware = new FakeCalibrationHardware((state, _) => Task.FromResult(
            new CalibrationMeasurement(
                Luminance: state.Brightness * 10,
                ChromaticityX: 0.31,
                ChromaticityY: 0.33,
                DeltaE: 1)));
        var progress = new List<CalibrationProgress>();

        var result = await _service.CalibrateBrightnessAsync(
            hardware,
            new BrightnessCalibrationInput(
                1,
                2,
                new HashSet<uint>(),
                WhitePoint(stablePasses: 1, maximumPasses: 1)),
            new RecordingProgress(progress));

        Assert.Equal(CalibrationCompletion.Converged, result.Completion);
        Assert.Collection(result.Points,
            point => Assert.Equal((1u, 10d), (point.Display.Brightness, point.Measurement.Luminance)),
            point => Assert.Equal((2u, 20d), (point.Display.Brightness, point.Measurement.Luminance)));
        Assert.Equal(2, progress.Count(p => p.Stage == CalibrationStage.MeasurementCompleted));
    }

    [Fact]
    public async Task ProbeErrorIsPropagatedWithoutInventingAMeasurement()
    {
        var expected = new InvalidOperationException("probe disconnected");
        var hardware = new FakeCalibrationHardware((_, _) => Task.FromException<CalibrationMeasurement>(expected));

        var actual = await Assert.ThrowsAsync<InvalidOperationException>(() =>
            _service.TuneWhitePointAsync(hardware, WhitePoint(1, 1)));

        Assert.Same(expected, actual);
        Assert.Equal(1, hardware.MeasurementCount);
    }

    [Fact]
    public async Task CancellationStopsAnInFlightHardwareRead()
    {
        var entered = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var hardware = new FakeCalibrationHardware(async (_, token) =>
        {
            entered.SetResult();
            await Task.Delay(Timeout.InfiniteTimeSpan, token);
            return default;
        });
        using var cancellation = new CancellationTokenSource();

        var operation = _service.TuneWhitePointAsync(
            hardware,
            WhitePoint(1, 10),
            cancellationToken: cancellation.Token);
        await entered.Task.WaitAsync(TimeSpan.FromSeconds(2));
        cancellation.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => operation);
    }

    [Fact]
    public async Task WhitePointReportsConvergenceAfterAStablePass()
    {
        var hardware = new FakeCalibrationHardware((_, _) => Task.FromResult(
            new CalibrationMeasurement(100, 0.31, 0.33, 1)));

        var result = await _service.TuneWhitePointAsync(
            hardware,
            WhitePoint(stablePasses: 1, maximumPasses: 1));

        Assert.Equal(CalibrationCompletion.Converged, result.Completion);
        Assert.Equal(1, result.PassCount);
    }

    [Fact]
    public async Task WhitePointReportsNonConvergenceAtThePassLimit()
    {
        var hardware = new FakeCalibrationHardware((state, _) => Task.FromResult(
            new CalibrationMeasurement(100, 0.31, 0.33, state.Gains.Red)));

        var result = await _service.TuneWhitePointAsync(
            hardware,
            WhitePoint(stablePasses: 2, maximumPasses: 1));

        Assert.Equal(CalibrationCompletion.NotConverged, result.Completion);
        Assert.Equal(1, result.PassCount);
        Assert.NotEqual(new CalibrationRgb(2, 2, 2), result.Gains);
    }

    static WhitePointCalibrationInput WhitePoint(int stablePasses, int maximumPasses) =>
        new(
            MaximumGain: 0,
            TestChannelPairs: false,
            SettleDelay: TimeSpan.Zero,
            StablePassesRequired: stablePasses,
            MaximumPasses: maximumPasses);

    sealed class RecordingProgress(List<CalibrationProgress> entries) : IProgress<CalibrationProgress>
    {
        public void Report(CalibrationProgress value) => entries.Add(value);
    }

    sealed class FakeCalibrationHardware(
        Func<CalibrationDisplayState, CancellationToken, Task<CalibrationMeasurement>> measure)
        : ICalibrationHardware
    {
        CalibrationDisplayState _state = new(0, 0, new(2, 2, 2));

        public CalibrationRange BrightnessRange { get; } = new(0, 10);
        public CalibrationRange ContrastRange { get; } = new(0, 10);
        public CalibrationRgbRanges GainRanges { get; } = new(
            new(0, 2), new(0, 2), new(0, 2));
        public CalibrationDisplayState CurrentState => _state;
        public int MeasurementCount { get; private set; }

        public Task SetBrightnessAsync(uint value, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            _state = _state with { Brightness = value };
            return Task.CompletedTask;
        }

        public Task SetContrastAsync(uint value, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            _state = _state with { Contrast = value };
            return Task.CompletedTask;
        }

        public Task SetGainsAsync(CalibrationRgb gains, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            _state = _state with { Gains = gains };
            return Task.CompletedTask;
        }

        public Task<CalibrationMeasurement> MeasureAsync(CancellationToken cancellationToken)
        {
            MeasurementCount++;
            return measure(_state, cancellationToken);
        }
    }
}
