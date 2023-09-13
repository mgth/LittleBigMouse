#pragma once
#include "HookMouseEventArg.h"
#include "Windows.h"
#include "ZonesLayout.h"
#include <mutex>

#include "Rect.h"
#include "ThreadHost.h"

class RemoteServer;


class MouseEngine : public ThreadHost
{

	RemoteServer* _remote = nullptr;

	geo::Point<long> _oldPoint = geo::Point<long>(0,0);
    Zone* _oldZone = nullptr;

	geo::Rect<long> _oldClipRect = geo::Rect<long>::Empty();

	std::mutex _lock;
	void (MouseEngine::*OnMouseMoveFunc)(HookMouseEventArg& e) = &MouseEngine::OnMouseMoveExtFirst;

	//First mouse event to init position
	void OnMouseMoveExtFirst(HookMouseEventArg& e);
	void SaveClip(const geo::Rect<long>& r);
	void ResetClip();

	//Mouse movement move least cpu usage strait between monitors
	void OnMouseMoveStraight(HookMouseEventArg& e);

	//Mouse movement taking care of direction, allows "corner crossing"
	void OnMouseMoveCross(HookMouseEventArg& e);

	//Final move
	void Move(HookMouseEventArg& e, const geo::Point<long>& pOut, const Zone* zoneOut);

	//Final move, cancel leaving monitor
	void NoZoneMatches(HookMouseEventArg& e);

	void RunThread() override;
	void DoStop() override;
	void OnStopped() override;

public:
	void Send(const std::string& message) const;
	void OnMouseMove(HookMouseEventArg& e);

	ZonesLayout Layout = ZonesLayout();

	void SetRemoteServer(RemoteServer* server) {_remote = server;}

};

