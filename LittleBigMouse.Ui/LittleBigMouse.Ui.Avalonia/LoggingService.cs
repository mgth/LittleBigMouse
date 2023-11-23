using System;
using Splat;

namespace LittleBigMouse.Ui.Avalonia;

public class LoggingService : ILogger
{
    public void Write(Exception exception, string message, Type type, LogLevel logLevel)
    {
        if (logLevel >= Level)
            System.Diagnostics.Debug.WriteLine(message);
    }

    public LogLevel Level { get; set; }

    public void Write(string message, LogLevel logLevel)
    {
        if (logLevel >= Level)
            System.Diagnostics.Debug.WriteLine(message);
    }

    public void Write(Exception exception, string message, LogLevel logLevel)
    {
        if (logLevel >= Level)
            System.Diagnostics.Debug.WriteLine(message);
    }

    public void Write(string message, Type type, LogLevel logLevel)
    {
        if (logLevel >= Level)
            System.Diagnostics.Debug.WriteLine(message);
    }
}