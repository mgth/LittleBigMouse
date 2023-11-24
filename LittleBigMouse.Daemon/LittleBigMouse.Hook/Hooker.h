#pragma once
#include <atomic>

#include "Priority.h"
#include "Windows.h"

#include "ThreadHost.h"
#include "nano_signal_slot.hpp"

class MouseEventArg;
class MouseEngine;

#define WM_CUSTOM_MESSAGE (WM_APP + 1)

class Hooker final : public ThreadHost
{
	static Hooker* _instance;

	DWORD _currentThreadId = 0;
	Priority _priority = Normal;

	HHOOK _mouseHookId = nullptr;
	HHOOK _windowHookId = nullptr;
	HWINEVENTHOOK _hEventHook = nullptr;
	HWND _hwnd = nullptr;

public:
	Nano::Signal<void(MouseEventArg&)> OnMouseMove;
	Nano::Signal<void(const std::wstring&)> OnWindowsChanged;
	Nano::Signal<void(const std::string&)> OnMessage;

	std::atomic_bool Stopping;

	HWND Hwnd() const { return _hwnd; }
	static Hooker* Instance() { return _instance; }

	void RunThread() override;
	void DoStop() override;
	void OnStopped() override;

	bool Hooked() const;

private:

	static LRESULT WINAPI MouseCallback(int nCode, WPARAM wParam, LPARAM lParam);
	static LRESULT WINAPI WindowCallback(int nCode, WPARAM wParam, LPARAM lParam);
	static void WindowChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hwnd, LONG idObject, LONG idChild,
	                             DWORD dwEventThread, DWORD dwmsEventTime);

	void SetPriority(const Priority priority) {_priority = priority;}

	void HookMouse();
	void UnhookMouse();

	void HookHiddenWindow();
	void UnhookHiddenWindow();

	void HookEvent();
	void UnhookEvent();

	void Loop();

	void HookWindows();
	void UnhookWindows();
};
