//// THIS CODE AND INFORMATION IS PROVIDED "AS IS" WITHOUT WARRANTY OF
//// ANY KIND, EITHER EXPRESSED OR IMPLIED, INCLUDING BUT NOT LIMITED TO
//// THE IMPLIED WARRANTIES OF MERCHANTABILITY AND/OR FITNESS FOR A
//// PARTICULAR PURPOSE.
////
//// Copyright (c) Microsoft Corporation. All rights reserved

#include "pch.h"
#include "PerMonitorDPIWindow.h"

namespace NativeHelpers 
{

	//Constructor; sets the current process as Per_Monitor_DPI_Aware
	PerMonitorDPIWindow::PerMonitorDPIWindow(void) 
	{		
		Loaded += gcnew System::Windows::RoutedEventHandler(this, &NativeHelpers::PerMonitorDPIWindow::OnLoaded);
		if (PerMonitorDPIHelper::SetPerMonitorDPIAware())
		{
			m_perMonitorEnabled = true;
		}
		else
		{
			throw gcnew System::Exception(L"Enabling Per-monitor DPI Failed.  Do you have [assembly: DisableDpiAwareness] in your assembly manifest [AssemblyInfo.cs]?");
		}

	}

	PerMonitorDPIWindow::~PerMonitorDPIWindow() 
	{
	}	

	//OnLoaded Handler: Adjusts the window size and graphics and text size based on current DPI of the Window

	void PerMonitorDPIWindow::OnLoaded(Object^ , RoutedEventArgs^ ) 
	{	
		// WPF has already scaled window size, graphics and text based on system DPI. In order to scale the window based on monitor DPI, update the 
		// window size, graphics and text based on monitor DPI. For example consider an application with size 600 x 400 in device independent pixels
		//		- Size in device independent pixels = 600 x 400 
		//		- Size calculated by WPF based on system/WPF DPI = 192 (scale factor = 2)
		//		- Expected size based on monitor DPI = 144 (scale factor = 1.5)

		// Similarly the graphics and text are updated updated by applying appropriate scale transform to the top level node of the WPF application

		// Important Note: This method overwrites the size of the window and the scale transform of the root node of the WPF Window. Hence, 
		// this sample may not work "as is" if 
		//	- The size of the window impacts other portions of the application like this WPF  Window being hosted inside another application. 
		//  - The WPF application that is extending this class is setting some other transform on the root visual; the sample may 
		//     overwrite some other transform that is being applied by the WPF application itself.
		
		
		
		
		if (m_perMonitorEnabled)
		{
			m_source = (HwndSource^) PresentationSource::FromVisual((Visual^) this);
			HwndSourceHook^ hook = gcnew HwndSourceHook(this, &PerMonitorDPIWindow::HandleMessages);
			m_source->AddHook(hook);	
			
			
			//Calculate the DPI used by WPF; this is same as the system DPI. 
					
			m_wpfDPI = 96.0 *  m_source->CompositionTarget->TransformToDevice.M11; 

			//Get the Current DPI of the monitor of the window. 
					
			m_currentDPI = NativeHelpers::PerMonitorDPIHelper::GetDpiForWindow(m_source->Handle);

			//Calculate the scale factor used to modify window size, graphics and text
			m_scaleFactor = m_currentDPI / m_wpfDPI; 
		
			//Update Width and Height based on the on the current DPI of the monitor
			
			Width = Width * m_scaleFactor;
			Height = Height * m_scaleFactor;

			//Update graphics and text based on the current DPI of the monitor
			
			UpdateLayoutTransform(m_scaleFactor);
		}
	}

	//Called when the DPI of the window changes. This method adjusts the graphics and text size based on the new DPI of the window
	void PerMonitorDPIWindow::OnDPIChanged() 
	{
		m_scaleFactor = m_currentDPI / m_wpfDPI;
		UpdateLayoutTransform(m_scaleFactor);
		DPIChanged(this, EventArgs::Empty);
	}


	void PerMonitorDPIWindow::UpdateLayoutTransform(double scaleFactor)
	{
		// Adjust the rendering graphics and text size by applying the scale transform to the top level visual node of the Window		
		if (m_perMonitorEnabled) 
		{		
			auto child = GetVisualChild(0);
			if (m_scaleFactor != 1.0) {
				ScaleTransform^ dpiScale = gcnew ScaleTransform(scaleFactor, scaleFactor);
				child->SetValue(Window::LayoutTransformProperty, dpiScale);
			}
			else 
			{
				child->SetValue(Window::LayoutTransformProperty, nullptr);
			}			
		}
	}

	
	// Message handler of the Per_Monitor_DPI_Aware window. The handles the WM_DPICHANGED message and adjusts window size, graphics and text
	// based on the DPI of the monitor. The window message provides the new window size (lparam) and new DPI (wparam)
	
	IntPtr PerMonitorDPIWindow::HandleMessages(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, bool% )
	{
		double oldDpi;
		switch (msg)
		{
			case WM_DPICHANGED:
				LPRECT lprNewRect = (LPRECT)lParam.ToPointer();
				SetWindowPos(static_cast<HWND>(hwnd.ToPointer()), 0, lprNewRect->left, lprNewRect->top, lprNewRect->right - lprNewRect->left, lprNewRect->bottom - lprNewRect->top, SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE);
				oldDpi = m_currentDPI;
				m_currentDPI = static_cast<int>(LOWORD(wParam.ToPointer()));
				if (oldDpi != m_currentDPI) 
				{
					OnDPIChanged();
				}
			break;
		}
		return IntPtr::Zero;
	}

	System::String^ PerMonitorDPIWindow::GetCurrentDpiConfiguration()
	{
		System::Text::StringBuilder^ stringBuilder = gcnew System::Text::StringBuilder();		

		auto awareness = NativeHelpers::PerMonitorDPIHelper::GetPerMonitorDPIAware();

		auto systemDpi = NativeHelpers::PerMonitorDPIHelper::GetSystemDPI();

		switch (awareness)
		{
		case PROCESS_DPI_AWARENESS::PROCESS_DPI_UNAWARE:
			
			stringBuilder->AppendFormat(gcnew System::String(L"Application is DPI Unaware.  Using {0} DPI."), systemDpi);
			break;
		case PROCESS_DPI_AWARENESS::PROCESS_SYSTEM_DPI_AWARE:
			stringBuilder->AppendFormat(gcnew System::String(L"Application is System DPI Aware.  Using System DPI:{0}."), systemDpi);
			break;
		case PROCESS_DPI_AWARENESS::PROCESS_PER_MONITOR_DPI_AWARE:			
			stringBuilder->AppendFormat(gcnew System::String(L"Application is Per-Monitor DPI Aware.  Using \tmonitor DPI = {0}  \t(System DPI = {1})."), m_currentDPI, systemDpi);
			break;
		}
		return stringBuilder->ToString();
	}
}