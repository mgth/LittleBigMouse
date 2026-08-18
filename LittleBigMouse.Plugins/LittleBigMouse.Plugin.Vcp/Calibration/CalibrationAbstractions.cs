namespace LittleBigMouse.Plugin.Vcp.Calibration;

/// <summary>
/// Narrow hardware boundary used by calibration. Implementations may use DDC/CI
/// and ArgyllCMS; tests can provide an in-memory display and probe.
/// </summary>
public interface ICalibrationHardware
{
    CalibrationRange BrightnessRange { get; }
    CalibrationRange ContrastRange { get; }
    CalibrationRgbRanges GainRanges { get; }
    CalibrationDisplayState CurrentState { get; }

    Task SetBrightnessAsync(uint value, CancellationToken cancellationToken);
    Task SetContrastAsync(uint value, CancellationToken cancellationToken);
    Task SetGainsAsync(CalibrationRgb gains, CancellationToken cancellationToken);
    Task<CalibrationMeasurement> MeasureAsync(CancellationToken cancellationToken);
}

public interface ICalibrationService
{
    Task<WhitePointCalibrationResult> TuneWhitePointAsync(
        ICalibrationHardware hardware,
        WhitePointCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default);

    Task<BrightnessCalibrationResult> CalibrateBrightnessAsync(
        ICalibrationHardware hardware,
        BrightnessCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default);

    Task<LowLuminanceCalibrationResult> CalibrateLowLuminanceAsync(
        ICalibrationHardware hardware,
        LowLuminanceCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default);

    Task<ContrastCalibrationResult> ProbeContrastAsync(
        ICalibrationHardware hardware,
        ContrastCalibrationInput input,
        IProgress<CalibrationProgress>? progress = null,
        CancellationToken cancellationToken = default);
}
