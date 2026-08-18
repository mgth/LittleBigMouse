namespace LittleBigMouse.Plugin.Vcp.Calibration;

public readonly record struct CalibrationRange(uint Min, uint Max)
{
    public uint Clamp(uint value) => Math.Clamp(value, Min, Max);
}

public readonly record struct CalibrationRgb(uint Red, uint Green, uint Blue)
{
    public uint Channel(int channel) => channel switch
    {
        0 => Red,
        1 => Green,
        2 => Blue,
        _ => throw new ArgumentOutOfRangeException(nameof(channel)),
    };

    public CalibrationRgb WithChannel(int channel, uint value) => channel switch
    {
        0 => this with { Red = value },
        1 => this with { Green = value },
        2 => this with { Blue = value },
        _ => throw new ArgumentOutOfRangeException(nameof(channel)),
    };

    public uint Min => Math.Min(Red, Math.Min(Green, Blue));
}

public readonly record struct CalibrationRgbRanges(
    CalibrationRange Red,
    CalibrationRange Green,
    CalibrationRange Blue)
{
    public CalibrationRange Channel(int channel) => channel switch
    {
        0 => Red,
        1 => Green,
        2 => Blue,
        _ => throw new ArgumentOutOfRangeException(nameof(channel)),
    };
}

public readonly record struct CalibrationDisplayState(
    uint Brightness,
    uint Contrast,
    CalibrationRgb Gains);

public readonly record struct CalibrationMeasurement(
    double Luminance,
    double ChromaticityX,
    double ChromaticityY,
    double DeltaE);

public sealed record CalibrationPoint(
    CalibrationDisplayState Display,
    CalibrationMeasurement Measurement);

public enum CalibrationCompletion
{
    Converged,
    NotConverged,
}

[Flags]
public enum CalibrationChannels
{
    None = 0,
    Red = 1,
    Green = 2,
    Blue = 4,
}

public enum CalibrationDirection
{
    Baseline,
    Down,
    Up,
    Revert,
}

public enum CalibrationStage
{
    Initializing,
    SettingBrightness,
    SettingContrast,
    AdjustingWhitePoint,
    Measuring,
    MeasurementCompleted,
    Completed,
}

public sealed record WhitePointAdjustment(
    CalibrationChannels Channels,
    CalibrationDirection Direction,
    double PreviousDeltaE,
    double CurrentDeltaE);

public sealed record CalibrationProgress(
    CalibrationStage Stage,
    int Current = 0,
    int Total = 0,
    CalibrationPoint? Point = null,
    WhitePointAdjustment? Adjustment = null);

public sealed record WhitePointCalibrationInput(
    uint MaximumGain,
    bool TestChannelPairs,
    TimeSpan SettleDelay,
    int StablePassesRequired = 6,
    int MaximumPasses = 120);

public sealed record BrightnessCalibrationInput(
    uint MinimumBrightness,
    uint MaximumBrightness,
    IReadOnlySet<uint> AlreadyMeasured,
    WhitePointCalibrationInput WhitePoint);

public sealed record LowLuminanceCalibrationInput(
    WhitePointCalibrationInput WhitePoint);

public sealed record ContrastCalibrationInput(
    uint MinimumContrast,
    uint MaximumContrast);

public sealed record WhitePointCalibrationResult(
    CalibrationCompletion Completion,
    CalibrationRgb Gains,
    double DeltaE,
    int MeasurementCount,
    int PassCount);

public sealed record BrightnessCalibrationResult(
    CalibrationCompletion Completion,
    IReadOnlyList<CalibrationPoint> Points);

public sealed record LowLuminanceCalibrationResult(
    CalibrationCompletion Completion,
    IReadOnlyList<CalibrationPoint> Points);

public sealed record ContrastCalibrationPoint(
    CalibrationChannels Channel,
    uint Contrast,
    CalibrationMeasurement Measurement);

public sealed record ContrastCalibrationResult(
    IReadOnlyList<ContrastCalibrationPoint> Points);

public sealed class CalibrationMeasurementException(string message) : Exception(message);
