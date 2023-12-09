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

	Priority _priority = Normal;
	bool _run = true;

	DWORD _currentThreadId = 0;

	HHOOK _mouseHookId = nullptr;
	HHOOK _displayHookId = nullptr;

	HWINEVENTHOOK _hEventFocusHook = nullptr;
	HWINEVENTHOOK _hEventDesktopHook = nullptr;

	HWND _hwnd = nullptr;

	bool _hookMouse = false;

	void HookMouse();
	void UnhookMouse();

	void HookDisplayChange();
	void UnhookDisplayChange();

	void HookFocusEvent();
	void UnhookFocusEvent();

	void HookEventSystemDesktopSwitch();
	void UnhookEventSystemDesktopSwitch();

	void HookWindows();
	void UnhookWindows();

	void DoHook();
	void DoUnhook();

	static void Loop();
	void QuitLoop();

public:

	Hooker();

	Nano::Signal<void(MouseEventArg&)> OnMouseMove;
	Nano::Signal<void(const std::string&)> OnFocusChanged;
	Nano::Signal<void()> OnDisplayChanged;
	Nano::Signal<void()> OnDesktopChanged;

	Nano::Signal<void()> OnHooked;
	Nano::Signal<void()> OnUnhooked;



	HWND Hwnd() const { return _hwnd; }
	static Hooker* Instance() { return _instance; }

	void RunThread() override;
	void DoStop() override;
	void OnStopped() override;

	bool Hooked() const;

	void SetPriority(const Priority priority) { _priority = priority; }

	void Hook() {
		_hookMouse = true;
		QuitLoop();
	}

	void Unhook() {
		_hookMouse = false;
		QuitLoop();
	}

private:
    static LRESULT __stdcall MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall DisplayChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall IniChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall WindowCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);

	static LRESULT DisplayChangeHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam);

	static void CALLBACK DesktopChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hWnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime);
	static void CALLBACK WindowChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hWnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime);
};
