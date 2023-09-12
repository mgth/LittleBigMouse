#include "MouseEngine.h"
#include "MouseHookerWindowsHook.h"
#include <iostream>
#include <windows.h>

#include "Geometry.h"
#include "tinyxml2.h"
#include "ZoneLink.h"
#include "RemoteServer.h"
#include "Zone.h"

void MouseEngine::OnMouseMoveExtFirst(HookMouseEventArg& e)
{
	_oldPoint = e.Point;

	_oldZone = Layout.Containing(_oldPoint);

	if (!_oldZone) return;

	switch (Layout.Algorithm)
	{
		case Strait :
			OnMouseMoveFunc = &MouseEngine::OnMouseMoveStraight;
			break;
		case CornerCrossing:
			OnMouseMoveFunc = &MouseEngine::OnMouseMoveCross;
	}
}


void MouseEngine::ResetClip(HookMouseEventArg& e)
{
	if (!_oldClipRect.IsEmpty())
	{
		MouseHookerWindowsHook::SetClip(_oldClipRect);
		_oldClipRect = geo::Rect<long>::Empty();
	}
}

// no zone found at mouse position
void MouseEngine::NoZoneMatches(HookMouseEventArg& e)
{
	// Store current clip zone to be restored at next move
	_oldClipRect = MouseHookerWindowsHook::GetClip();
	// _reset = true;

	// Clip to current zone to get cursor back
	MouseHookerWindowsHook::SetClip(_oldZone->PixelsBounds());

	ResetClip(e);

	e.Handled = false;// when set to true, cursor stick to frame
}

void MouseEngine::OnMouseMoveCross(HookMouseEventArg& e)
{
	const auto pIn = e.Point;
	ResetClip(e);

	if (_oldZone->PixelsBounds().Contains(pIn))
	{
		_oldPoint = pIn;
		e.Handled = false;
		return;
	}

	const auto pInMmOld = _oldZone->ToPhysical(_oldPoint);
	const auto pInMm = _oldZone->ToPhysical(pIn);
	const Zone* zoneOut = nullptr;

	//Get line from previous point to current point
	const auto trip = geo::Segment<double>(pInMmOld, pInMm).Line();
	auto minDist = DBL_MAX;

	auto pOutInMm = pInMm;

	// Check all zones for borders intersections with mouse direction.
	for (const auto zone : Layout.Zones)
	{
		// exclude zone we are currently in.
		if (_oldZone == zone) continue;
		for (auto p : zone->PhysicalBounds().Intersect(trip))
		{
			auto travelBox = geo::Rect<double>(pInMmOld,p);

			// check if good direction
			if(!travelBox.Contains(pInMm)) continue;

			// calculate distance (squared) to retain the intersection with minimal travel distance
			const auto dist = pow(travelBox.Width(),2) + pow(travelBox.Height(),2);

			if (dist > minDist) continue;

			minDist = dist;
			zoneOut = zone;
			pOutInMm = p;
		}
	}


	if (zoneOut == nullptr || minDist > pow(100,2) )
	{
		NoZoneMatches(e);
		return;
	}

	const auto pOut = zoneOut->ToPixels(pOutInMm);

	Move(e, pIn, pOut, zoneOut);
}


void MouseEngine::OnMouseMoveStraight(HookMouseEventArg& e)
{
	const auto pIn = e.Point;
	ResetClip(e);

	const ZoneLink* zoneOut;
	geo::Point<long> pOut;
	const auto bounds = _oldZone->PixelsBounds();
	// leaving zone by right
	if (pIn.X() >= bounds.Right())
	{
		zoneOut = _oldZone->RightZones->AtPixel(pIn.Y());
		if (zoneOut->Target)
		{
			pOut = {zoneOut->Target->PixelsBounds().Left(),zoneOut->ToTargetPixel(pIn.Y())};
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by left
	else if (pIn.X() < bounds.Left())
	{
		zoneOut = _oldZone->LeftZones->AtPixel(pIn.Y());
		if (zoneOut->Target)
		{
			pOut = {zoneOut->Target->PixelsBounds().Right() - 1,zoneOut->ToTargetPixel(pIn.Y())};
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by bottom
	else if (pIn.Y() >= bounds.Bottom())
	{
		zoneOut = _oldZone->BottomZones->AtPixel(pIn.Y());
		if (zoneOut->Target)
		{
			pOut = {zoneOut->ToTargetPixel(pIn.X()), zoneOut->Target->PixelsBounds().Top()};
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by top
	else if (pIn.Y() < _oldZone->PixelsBounds().Top())
	{
		zoneOut = _oldZone->TopZones->AtPixel(pIn.X());
		if (zoneOut->Target)
		{
			pOut = {zoneOut->ToTargetPixel(pIn.X()),zoneOut->Target->PixelsBounds().Bottom() - 1};
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	else
	{
		_oldPoint = pIn;
		e.Handled = false;
		return;
	}

#ifdef _DEBUG_
	std::cout << "to : " << zoneOut->Target->Name << " at " << pOut.X() << "," << pOut.X() /*<< "(" << pInMm.X() << "," << pInMm.Y() << ")\n"*/;
#endif
	Move(e, pIn,pOut,zoneOut->Target);
}

void MouseEngine::Move(HookMouseEventArg& e, const geo::Point<long>& pIn, const geo::Point<long>& pOut, const Zone* zoneOut)
{
e.StartTiming();

	const auto travel = _oldZone->TravelPixels(Layout.MainZones, zoneOut);

	_oldZone = zoneOut->Main;
	_oldPoint = pOut;

	const auto r = zoneOut->PixelsBounds();

	_oldClipRect = MouseHookerWindowsHook::GetClip();
	auto pos = pIn;

	for (const auto& rect : travel)
	{

		if (rect.Contains(pos)) continue;

		MouseHookerWindowsHook::SetClip(rect);

		pos = MouseHookerWindowsHook::GetMouseLocation();

		if (rect.Contains(pOut)) break;
	}

	MouseHookerWindowsHook::SetClip(r);
	MouseHookerWindowsHook::SetMouseLocation(pOut);


	_oldClipRect = MouseHookerWindowsHook::GetClip();
	MouseHookerWindowsHook::SetMouseLocation(pOut);

	e.Handled = true;

	e.EndTiming();

}

void MouseEngine::RunThread()
{
	MouseHookerWindowsHook::Start(*this);
}

void MouseEngine::Send(const std::string& message) const
{
	if(_remote) _remote->Send(message);
}

void MouseEngine::OnMouseMove(HookMouseEventArg& e)
{
	e.StartTiming();
	_lock.lock();
	(this->*OnMouseMoveFunc)(e);
	_lock.unlock();

	if(e.Timing())
	{
		std::cout << e.GetDuration() << "\n";
	}
}
void MouseEngine::DoStop()
{
		MouseHookerWindowsHook::Stopping = true;
}

void MouseEngine::OnStopped()
{
		OnMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
}
