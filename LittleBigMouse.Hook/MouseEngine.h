#pragma once
#include "HookMouseEventArg.h"
//#include "MouseHookerWindowsHook.h"
#include "Windows.h"
#include "ZonesLayout.h"
#include <mutex>

#include "RemoteServer.h"

class MouseEngine
{



	POINT _oldPoint = POINT{0,0};
    Zone* _oldZone = nullptr;

	bool _reset = false;

	RECT _oldClipRect = RECT{0,0,0,0};

	std::mutex _lock;
	void (MouseEngine::*OnMouseMoveFunc)(HookMouseEventArg& e) = &MouseEngine::OnMouseMoveExtFirst;

	void OnMouseMoveExtFirst(HookMouseEventArg& e);

	void OnMouseMoveStraight(HookMouseEventArg& e);
	//void OnMouseMoveCross(HookMouseEventArg& e);
	void StartThread();

	std::thread* _thread = nullptr;

	void Send(const std::string& message) const
	{
		if(Remote) Remote->Send(message);
	}

public:
	ZonesLayout Layout = ZonesLayout();

	RemoteServer* Remote;

	void OnMouseMove(HookMouseEventArg& e);

	void Start();
	void Stop();
};

