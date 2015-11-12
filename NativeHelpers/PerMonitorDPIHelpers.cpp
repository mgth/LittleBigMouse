//// THIS CODE AND INFORMATION IS PROVIDED "AS IS" WITHOUT WARRANTY OF
//// ANY KIND, EITHER EXPRESSED OR IMPLIED, INCLUDING BUT NOT LIMITED TO
//// THE IMPLIED WARRANTIES OF MERCHANTABILITY AND/OR FITNESS FOR A
//// PARTICULAR PURPOSE.
////
//// Copyright (c) Microsoft Corporation. All rights reserved	


//Main helper class that provides helper functions used by the PerMonitorDPIWindow class

#include "pch.h"
#include "PerMonitorDPIHelpers.h"


// Returns the DPI awareness of the current process

PROCESS_DPI_AWARENESS NativeHelpers::PerMonitorDPIHelper::GetPerMonitorDPIAware()
{

	PROCESS_DPI_AWARENESS awareness;
	HANDLE hProcess;
	hProcess = OpenProcess(PROCESS_ALL_ACCESS, false, GetCurrentProcessId());
	auto result = GetProcessDpiAwareness(hProcess, &awareness);

	if (S_OK != result)
	{
		throw gcnew System::Exception(L"Unable to read process DPI level");
	}
	return awareness;
}

//Sets the current process as Per_Monitor_DPI_Aware. Returns True if the process was marked as Per_Monitor_DPI_Aware

BOOL NativeHelpers::PerMonitorDPIHelper::SetPerMonitorDPIAware()
{
	auto result = SetProcessDpiAwareness(PROCESS_DPI_AWARENESS::PROCESS_PER_MONITOR_DPI_AWARE);

	if (S_OK != result)
	{
		return FALSE;
	}
	return TRUE;
}

//Returns the DPI of the window handle passed in the parameter 

double NativeHelpers::PerMonitorDPIHelper::GetDpiForWindow(IntPtr hwnd)
{
	return GetDpiForHwnd(static_cast<HWND>(hwnd.ToPointer()));
}

double NativeHelpers::PerMonitorDPIHelper::GetDpiForHwnd(HWND hWnd)
{
	auto monitor = MonitorFromWindow(hWnd, MONITOR_DEFAULTTONEAREST);
	UINT newDpiX;
	UINT newDpiY;
	if (FAILED(GetDpiForMonitor(monitor, MONITOR_DPI_TYPE::MDT_EFFECTIVE_DPI, &newDpiX, &newDpiY)))
	{
		newDpiX = 96;
		newDpiY = 96;
	}
	return ((double) newDpiX);
}


//Returns the system DPI

double NativeHelpers::PerMonitorDPIHelper::GetSystemDPI()
{
	int newDpiX(0);
	auto hDC = GetDC(NULL);
	newDpiX = GetDeviceCaps(hDC, LOGPIXELSX);
	ReleaseDC(NULL, hDC);
	return (double)newDpiX;
}
