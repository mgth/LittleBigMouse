#pragma once
#include <atomic>

#include "Windows.h"

#include "Rect.h"

class MouseEngine;

class MouseHookerWindowsHook
{
	static MouseEngine* _engine;

public:
	static HHOOK HookId;
	static std::atomic_bool Stopping;

	static int Start(MouseEngine& engine);
	static bool Hooked();
	static MouseEngine* GetEngine() {return _engine;}

	static MSG Msg; // struct with information about all messages in our queue

	static void SetMouseLocation(const geo::Point<long>& location);
	static geo::Point<long> GetMouseLocation();
	static void SetClip(const geo::Rect<long>& r);
	static geo::Rect<long> GetClip();

};

LRESULT WINAPI MouseCallback(int nCode, WPARAM wParam, LPARAM lParam);

