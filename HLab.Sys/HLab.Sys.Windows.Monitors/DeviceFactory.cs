using HLab.Sys.Windows.API;
using System;
using static HLab.Sys.Windows.API.WinGdi;


namespace HLab.Sys.Windows.Monitors;

internal static class DeviceFactory
{
    public static MonitorDevice BuildMonitorDevice(DisplayDevice parent, WinGdi.DisplayDevice device)
    {
        var edid = MonitorDeviceHelper.GetEdid(device.DeviceID);
        var monitor = new MonitorDevice
        {
            Parent = parent as PhysicalAdapter,

            //DisplayDevice
            DeviceId = device.DeviceID,
            DeviceName = device.DeviceName,
            DeviceString = device.DeviceString,
            DeviceKey = device.DeviceKey,

            State = MonitorDeviceHelper.BuildDeviceState(device.StateFlags),
            Capabilities = MonitorDeviceHelper.BuildDeviceCaps(device.DeviceName),
            CurrentMode = MonitorDeviceHelper.GetCurrentMode(device.DeviceName),

            //MonitorDevice
            Edid = edid,
            PnpCode = MonitorDeviceHelper.GetPnpCodeFromId(device.DeviceID),
            PhysicalId = MonitorDeviceHelper.GetPhysicalId(device.DeviceID, edid),
            SourceId = MonitorDeviceHelper.GetSourceId(device.DeviceID, edid),
        };

        monitor.UpdateDevModes();

        return monitor;
    }


    public static PhysicalAdapter BuildPhysicalAdapter(DisplayDevice parent, WinGdi.DisplayDevice device)
    {
        var adapter = new PhysicalAdapter
        {
            Parent = parent,
            //DisplayDevice
            DeviceId = device.DeviceID,
            DeviceName = device.DeviceName,
            DeviceString = device.DeviceString,
            DeviceKey = device.DeviceKey,

            State = MonitorDeviceHelper.BuildDeviceState(device.StateFlags),
            Capabilities = MonitorDeviceHelper.BuildDeviceCaps(device.DeviceName),

            CurrentMode = MonitorDeviceHelper.GetCurrentMode(device.DeviceName),
        };

        adapter.UpdateDevModes();

        return adapter;
    }

    public static DisplayDevice BuildDisplayDeviceAndChildren(
        DisplayDevice? parent,
        WinGdi.DisplayDevice dev
        )
    {
        var result = BuildDisplayDevice(parent, dev);

        uint i = 0;
        var child = new WinGdi.DisplayDevice();

        while (EnumDisplayDevices(result.DeviceName, i++, ref child, 0))
        {
            result.AddChild(BuildDisplayDeviceAndChildren(result, child));
        }
        return result;
    }

    static DisplayDevice BuildDisplayDevice(DisplayDevice? parent, WinGdi.DisplayDevice device)
    {
        switch (device.DeviceID.Split('\\')[0])
        {
            case "ROOT":
                return new DisplayDevice
                {
                    Parent = parent,
                    //DisplayDevice
                    DeviceId = device.DeviceID,
                    DeviceName = device.DeviceName,
                    DeviceString = device.DeviceString,
                    DeviceKey = device.DeviceKey,

                    State = MonitorDeviceHelper.BuildDeviceState(device.StateFlags),
                    Capabilities = MonitorDeviceHelper.BuildDeviceCaps(device.DeviceName),

                    CurrentMode = MonitorDeviceHelper.GetCurrentMode(device.DeviceName),
                };

            case "MONITOR":

                return BuildMonitorDevice(parent, device);

            case "RdpIdd_IndirectDisplay":
            case string s when s.StartsWith("VID_DATRONICSOFT_PID_SPACEDESK_VIRTUAL_DISPLAY_"):
            case "PCI":

                return BuildPhysicalAdapter(parent, device);

            default:
                break;
        }

        throw new ArgumentException($"Unknown device type {device.DeviceName}");

    }

}
