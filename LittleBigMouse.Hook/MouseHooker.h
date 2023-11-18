#pragma once
#include <atomic>

#include "Priority.h"
#include "Windows.h"

#include "Rect.h"
#include "ThreadHost.h"
#include "nano_signal_slot.hpp"

class MouseEventArg;
class MouseEngine;

#define WM_CUSTOM_MESSAGE WM_APP + 1


class MouseHooker final : public ThreadHost
{
	static MouseHooker* _instance;

	DWORD _currentThreadId = 0;
	Priority _priority = Normal;

	HHOOK _mouseHookId = nullptr;
	HHOOK _windowHookId = nullptr;
	static HWINEVENTHOOK _hEventHook;

public:
	Nano::Signal<void(MouseEventArg&)> OnMouseMove;
	Nano::Signal<void()> OnWindowsChanged;
	Nano::Signal<void(const std::string&)> OnMessage;

	std::atomic_bool Stopping;

	static MouseHooker* Instance() { return _instance; }

	int Hook();
	void SetPriority(const Priority priority) {_priority = priority;}


	void RunThread() override;
	void DoStop() override;
	void OnStopped() override;

	bool Hooked() const;

	static LRESULT WINAPI MouseCallback(int nCode, WPARAM wParam, LPARAM lParam);
	static LRESULT WINAPI WindowCallback(int nCode, WPARAM wParam, LPARAM lParam);
	static void WindowChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hwnd, LONG idObject, LONG idChild,
	                      DWORD dwEventThread, DWORD dwmsEventTime);
};


