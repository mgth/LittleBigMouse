using System.Text;
using HLab.Geo;
using HLab.Sys.Windows.Monitors;
using HLab.Sys.Windows.Monitors.Factory;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Platform.Windows;
using Xunit;

namespace LittleBigMouse.DisplayLayout.Tests;

public class WindowsMonitorSafetyTests
{
    [Fact]
    public void KeyNameInformationUsesByteLengthWithoutReadingPastBuffer()
    {
        const string path = @"\REGISTRY\MACHINE\SYSTEM\Monitor";
        var name = Encoding.Unicode.GetBytes(path);
        var data = new byte[sizeof(uint) + name.Length];
        BitConverter.TryWriteBytes(data.AsSpan(), (uint)name.Length);
        name.CopyTo(data, sizeof(uint));

        Assert.Equal(path, MonitorDeviceHelper.DecodeKeyNameInformation(data));

        BitConverter.TryWriteBytes(data.AsSpan(), (uint)(name.Length + 2));
        Assert.Throws<InvalidDataException>(() => MonitorDeviceHelper.DecodeKeyNameInformation(data));
    }

    [Fact]
    public void ActiveConnectionWinsRegardlessOfEnumerationOrder()
    {
        var stale = Connection(@"\.\DISPLAY9", attached: false, hMonitor: 0);
        var active = Connection(@"\.\DISPLAY2", attached: true, hMonitor: 42);

        Assert.Same(active, MonitorDevice.SelectConnection([stale, active]));
        Assert.Same(active, MonitorDevice.SelectConnection([active, stale]));
    }

    [Fact]
    public void LocationBatchRollsBackWithoutCommitAfterStageFailure()
    {
        var transaction = new FakeTransaction(stageResults: [true, false], commit: true);
        var controller = new WindowsDisplayController(transaction);

        var result = controller.SetLocations([
            (Source("one", 0), new Point(100, 0), null),
            (Source("two", 1920), new Point(2020, 0), null)
        ]);

        Assert.False(result);
        Assert.Equal(2, transaction.StageCalls);
        Assert.Equal(0, transaction.CommitCalls);
        Assert.Equal(1, transaction.RestoreCalls);
    }

    [Fact]
    public void LocationBatchRollsBackWhenCommitFails()
    {
        var transaction = new FakeTransaction(stageResults: [true], commit: false);
        var controller = new WindowsDisplayController(transaction);

        var result = controller.SetLocations([
            (Source("one", 0), new Point(100, 0), null)
        ]);

        Assert.False(result);
        Assert.Equal(1, transaction.CommitCalls);
        Assert.Equal(1, transaction.RestoreCalls);
    }

    static MonitorDeviceConnection Connection(string name, bool attached, nint hMonitor)
        => new()
        {
            DeviceName = name,
            State = new DeviceState { AttachedToDesktop = attached },
            Parent = new PhysicalAdapter
            {
                DeviceName = name,
                HMonitor = hMonitor,
                CurrentMode = attached ? new DisplayMode() : null!
            }
        };

    static DisplaySource Source(string id, double x)
    {
        var source = new DisplaySource(id) { InterfacePath = $@"\?\DISPLAY#{id}" };
        source.InPixel.Set(new Rect(x, 0, 1920, 1080));
        return source;
    }

    sealed class FakeTransaction(IEnumerable<bool> stageResults, bool commit)
        : IWindowsDisplayTransaction
    {
        readonly Queue<bool> _stageResults = new(stageResults);
        public int StageCalls { get; private set; }
        public int CommitCalls { get; private set; }
        public int RestoreCalls { get; private set; }

        public bool Stage(DisplaySource source, Rect bounds)
        {
            StageCalls++;
            return _stageResults.Dequeue();
        }

        public bool Commit()
        {
            CommitCalls++;
            return commit;
        }

        public bool Restore()
        {
            RestoreCalls++;
            return true;
        }
    }
}
