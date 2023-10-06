#pragma once
#include "HookMouseEventArg.h"
#include "ZonesLayout.h"
#include <mutex>

#include "SignalSlot.h"
#include "Segment.h"

class MouseEngine 
{
	geo::Point<long> _oldPoint = geo::Point<long>(0,0);
    Zone* _oldZone = nullptr;

	geo::Rect<long> _oldClipRect = geo::Rect<long>::Empty();

	std::mutex _lock;
	void (MouseEngine::*OnMouseMoveFunc)(MouseEventArg& e) = &MouseEngine::OnMouseMoveExtFirst;

	//First mouse event to init position
	void OnMouseMoveExtFirst(MouseEventArg& e);
	void SaveClip(const geo::Rect<long>& r);
	void ResetClip();

	//Mouse movement move least cpu usage strait between monitors
	void OnMouseMoveStraight(MouseEventArg& e);

	//Mouse movement taking care of direction, allows "corner crossing"
	void OnMouseMoveCross(MouseEventArg& e);

	//Final move
	void Move(MouseEventArg& e, const geo::Point<long>& pOut, const Zone* zoneOut);
	void MoveInMm(MouseEventArg& e, const geo::Point<double>& pOutInMm, const Zone* zoneOut);

	//Final move, cancel leaving monitor
	void NoZoneMatches(MouseEventArg& e);
	Zone* FindTargetZone(const Zone* current, const geo::Segment<double>& trip, geo::Point<double>& pOutInMm, double minDist) const;

public:
	Signal<std::string&> OnMessage;

	void Reset();
	void OnMouseMove(MouseEventArg& e);

	ZonesLayout Layout = ZonesLayout();

};

