#pragma once
#include "Windows.h"

#include "MouseEngine.h"

class MouseHookerWindowsHook
{
	static MouseEngine* _engine;

public:
	static HHOOK HookId;
	static std::atomic_bool Stop;

	static int Start(MouseEngine& engine);
	static bool Hooked();
	static MouseEngine* GetEngine() {return _engine;}

	static MSG Msg; // struct with information about all messages in our queue

};

LRESULT WINAPI MouseCallback(int nCode, WPARAM wParam, LPARAM lParam);

