#pragma once
#include <atomic>

#include "Windows.h"

#include "Rect.h"
#include "ThreadHost.h"
#include "SignalSlot.h"

class MouseEventArg;
class MouseEngine;

class MouseHooker final : public ThreadHost
{
	static MouseHooker* _instance;

public:
	Signal<MouseEventArg&> OnMouseMove;
	Signal<> OnWindowsChanged;
	Signal<const std::string&> OnMessage;

	HHOOK MouseHookId;
	HHOOK WindowHookId;
	std::atomic_bool Stopping;

	static MouseHooker* Instance() { return _instance; }

	int Hook();

	void RunThread() override;
	void DoStop() override;
	void OnStopped() override;

	bool Hooked() const;

	static LRESULT WINAPI MouseCallback(int nCode, WPARAM wParam, LPARAM lParam);
	static LRESULT WINAPI WindowCallback(int nCode, WPARAM wParam, LPARAM lParam);
};


