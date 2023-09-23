// See https://aka.ms/new-console-template for more information
using HLab.Sys.Windows.API;
using static HLab.Sys.Windows.API.WinUser;

Console.WriteLine("Hello, World!");

WinUser.SystemParametersInfo(WinUser.SystemParametersInfoAction.SetDeskWallpaper,
    0,
    0,
    SystemParametersInfoFlags.UpdateIniFile | SystemParametersInfoFlags.SendWinIniChange);
