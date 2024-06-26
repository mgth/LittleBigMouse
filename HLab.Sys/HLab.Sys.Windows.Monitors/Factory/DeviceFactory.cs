﻿using HLab.Sys.Windows.API;
using System;
using System.Diagnostics;
using System.Linq;
using static HLab.Sys.Windows.API.WinGdi;


namespace HLab.Sys.Windows.Monitors.Factory;

internal static class DeviceFactory
{
    public static MonitorDeviceConnection BuildMonitorDevice(DisplayDevice parent, WinGdi.DisplayDevice device)
    {

        var monitors = parent.Parent.AllMonitorDevices().ToList();

        var monitor = monitors.FirstOrDefault(m => m.Id == device.DeviceID);

        if (monitor == null)
        {
            var edid = MonitorDeviceHelper.GetEdid(device.DeviceID);

            monitor = new MonitorDevice
            {
                Id = device.DeviceID,
                //MonitorDevice
                Edid = edid,
                PnpCode = MonitorDeviceHelper.GetPnpCodeFromId(device.DeviceID),
                PhysicalId = MonitorDeviceHelper.GetPhysicalId(device.DeviceID, edid),
                SourceId = MonitorDeviceHelper.GetSourceId(device.DeviceID, edid),
            };
        }

        var connection = new MonitorDeviceConnection
        {
            Parent = parent as PhysicalAdapter,
            Monitor = monitor,

            //DisplayDevice
            Id = device.DeviceID,
            DeviceName = device.DeviceName,
            DeviceString = device.DeviceString,
            DeviceKey = device.DeviceKey,

            State = MonitorDeviceHelper.BuildDeviceState(device.StateFlags),
            Capabilities = MonitorDeviceHelper.BuildDeviceCaps(device.DeviceName),
            CurrentMode = MonitorDeviceHelper.GetCurrentMode(device.DeviceName),
        };

        monitor.Connections.Add(connection);

        connection.UpdateDevModes();

        return connection;
    }


    public static PhysicalAdapter BuildPhysicalAdapter(DisplayDevice parent, WinGdi.DisplayDevice device)
    {
        var adapter = new PhysicalAdapter
        {
            Parent = parent,
            //DisplayDevice
            Id = device.DeviceID,
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
        DisplayDevice parent,
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
        if(device.DeviceID == "ROOT")
        {
            return new DisplayDevice
            {
                Parent = parent,
                //DisplayDevice
                Id = device.DeviceID,
                DeviceName = device.DeviceName,
                DeviceString = device.DeviceString,
                DeviceKey = device.DeviceKey,

                State = MonitorDeviceHelper.BuildDeviceState(device.StateFlags),
                Capabilities = MonitorDeviceHelper.BuildDeviceCaps(device.DeviceName),

                CurrentMode = MonitorDeviceHelper.GetCurrentMode(device.DeviceName),
            };
        }

        Debug.Assert(parent != null);

        if (parent.Id=="ROOT")
            return BuildPhysicalAdapter(parent, device);

        //if(device.DeviceID.Split('\\')[0] == "MONITOR")
            return BuildMonitorDevice(parent, device);

        throw new ArgumentException($"Unknown device type {device.DeviceName}");

    }

}
