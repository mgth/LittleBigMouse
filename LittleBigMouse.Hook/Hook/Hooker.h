#pragma once
#include "framework.h"
#include <atomic>

#include "SignalSlot.h"
#include "Engine/Priority.h"
#include "Thread/ThreadHost.h"

class MouseEventArg;
class MouseEngine;

#define WM_BREAK_LOOP (WM_APP + 1)

class Hooker final
{
	static std::atomic<Hooker*> _instance;

	Priority _priority = Normal;
	Priority _priorityUnhooked = Below;

	DWORD _currentThreadId = 0;

	HHOOK _mouseHookId = nullptr;
	HHOOK _displayHookId = nullptr;

	HWINEVENTHOOK _hEventFocusHook = nullptr;
	HWINEVENTHOOK _hEventDesktopHook = nullptr;

	HWND _hwnd = nullptr;

	bool _hookMouse = false;
	HINSTANCE _hInst = nullptr;

	void HookMouse();
	void UnhookMouse();


	bool HookDisplayChange();
	void UnhookDisplayChange();

	void HookFocusEvent();
	void UnhookFocusEvent();

	void HookEventSystemDesktopSwitch();
	void UnhookEventSystemDesktopSwitch();

	void HookWindows();
	void UnhookWindows();

	static bool PumpMessages();

	void BreakLoop() const;

	static ATOM RegisterClassLbm(HINSTANCE hInstance);
	BOOL InitInstance(HINSTANCE hInstance);

public:

	void DoHook();
	void DoUnhook();

	SIGNAL<void(MouseEventArg&)> OnMouseMove;
	SIGNAL<void(const std::string&)> OnFocusChanged;
	SIGNAL<void()> OnDisplayChanged;
	SIGNAL<void()> OnDesktopChanged;

	SIGNAL<void()> OnHooked;
	SIGNAL<void()> OnUnhooked;

	HWND Hwnd() const { return _hwnd; }
	static Hooker* Instance() { return _instance.load(); }

	void Loop();

	bool Hooked() const;

	void SetPriority(const Priority priority) { _priority = priority; }
	void SetPriorityUnhooked(const Priority priority) { _priorityUnhooked = priority; }

	void Hook();

	void Unhook();

	void Quit() const;

private:
    static LRESULT __stdcall MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall DisplayChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall IniChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);
	static LRESULT __stdcall WindowCallback(const int nCode, const WPARAM wParam, const LPARAM lParam);

	static LRESULT CALLBACK DisplayChangeHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam);

	static void CALLBACK DesktopChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hWnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime);
	static void CALLBACK WindowChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hWnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime);
};
