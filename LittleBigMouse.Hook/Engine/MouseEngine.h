#pragma once
#include "Framework.h"

#include <mutex>

#include "SignalSlot.h"
#include "HookMouseEventArg.h"
#include "ZonesLayout.h"
#include "Geometry/Rect.h"
#include "Geometry/Segment.h"

class ZoneLink;

class MouseEngine 
{

	geo::Point<long> _oldPoint = geo::Point<long>(0,0);
    Zone* _oldZone = nullptr;

	geo::Rect<long> _oldClipRect = geo::Rect<long>::Empty();

	std::mutex _lock;
	void (MouseEngine::*_onMouseMoveFunc)(MouseEventArg& e) = &MouseEngine::OnMouseMoveExtFirst;

	//First mouse event to init position
	void OnMouseMoveExtFirst(MouseEventArg& e);

	//save current clip cursor
	void SaveClip();

	//reset clip cursor from saved
	void ResetClip();

	//Mouse movement move least cpu usage strait between monitors
	void OnMouseMoveStraight(MouseEventArg& e);

	//Mouse movement taking care of direction, allows "corner crossing"
	void OnMouseMoveCross(MouseEventArg& e);
	bool TryPassBorder(const ZoneLink* zoneLink, const double distance);
	bool TryPassBorderPixel(const ZoneLink* zoneLink, long distance);

	//Final move
	void Move(MouseEventArg& e, const geo::Point<long>& pOut, const Zone* zoneOut);
	void MoveInMm(MouseEventArg& e, const geo::Point<double>& pOutInMm, const Zone* zoneOut);

	//Final move, cancel leaving monitor
	void NoZoneMatches(MouseEventArg& e);

	//Find first zone intersecting with mouse direction
	Zone* FindTargetZone(const Zone* current, const geo::Segment<double>& trip, geo::Point<double>& pOutInMm, double minDistSquared) const;
	bool CheckForStopped(const MouseEventArg& e);
	bool TryPassBorderCross(const Zone* zone, const geo::Segment<double>& trip);

	const ZoneLink* _currentResistanceLink = nullptr;
	double _borderResistance = 0;
	long _borderResistancePixel = 0;

public:
	SIGNAL<void(std::string&)> OnMessage;

#if defined(_DEBUG_)
	SIGNAL<void()> DebugUnhook;
#endif

	void Reset();
	void OnMouseMove(MouseEventArg& e);

	ZonesLayout Layout = ZonesLayout();

};

