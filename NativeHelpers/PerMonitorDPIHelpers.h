//// THIS CODE AND INFORMATION IS PROVIDED "AS IS" WITHOUT WARRANTY OF
//// ANY KIND, EITHER EXPRESSED OR IMPLIED, INCLUDING BUT NOT LIMITED TO
//// THE IMPLIED WARRANTIES OF MERCHANTABILITY AND/OR FITNESS FOR A
//// PARTICULAR PURPOSE.
////
//// Copyright (c) Microsoft Corporation. All rights reserved

#pragma once
#include "pch.h"

using namespace System;

namespace NativeHelpers 
{
	public ref class PerMonitorDPIHelper
	{
	public:
		
		static BOOL SetPerMonitorDPIAware();

		static PROCESS_DPI_AWARENESS GetPerMonitorDPIAware();
		
		static double GetDpiForWindow(IntPtr hwnd);

		static double GetSystemDPI();	
		
	private:
		static double GetDpiForHwnd(HWND hWnd);		
	};
}
