#include "MouseEngine.h"

#include "../Geometry/Geometry.h"

#include "MouseHelper.h"
#include "ZoneLink.h"
#include "Zone.h"

void MouseEngine::OnMouseMoveExtFirst(MouseEventArg& e)
{
	if(CheckForStopped(e)) return;

	_oldPoint = e.Point;

	_oldZone = Layout.Containing(_oldPoint);

	if (!_oldZone) return;

	switch (Layout.Algorithm)
	{
		case Strait:
			_onMouseMoveFunc = &MouseEngine::OnMouseMoveStraight;
			break;

		case CornerCrossing:
			_onMouseMoveFunc = &MouseEngine::OnMouseMoveCross;
			break;
	}
}

void MouseEngine::SaveClip()
{
#if _DEBUG_
	if (!_oldClipRect.IsEmpty())
	{
		LOG_DEBUG("/!\\ Saved clip override");
	}
#endif
	_oldClipRect = GetClip();
}

void MouseEngine::ResetClip()
{
	if (!_oldClipRect.IsEmpty())
	{
		//LOG_TRACE("<engine:ResetClip>");
		SetClip(_oldClipRect);
		_oldClipRect = geo::Rect<long>::Empty();
	}
}

// no zone found at mouse position
void MouseEngine::NoZoneMatches(MouseEventArg& e)
{
	// Store current clip zone to be restored at next move
	// Clip to current zone to get cursor back
	SaveClip();
	SetClip(_oldZone->PixelsBounds());

	e.Handled = false;// when set to true, cursor stick to frame

#ifdef _DEBUG_
	const auto p = _oldZone->ToPhysical(e.Point);
	LOG_TRACE_1("NoZoneMatches : " << _oldZone->Name << e.Point << p );
#endif

}

Zone* MouseEngine::FindTargetZone(const Zone* current, const geo::Segment<double>& trip, geo::Point<double>& pOutInMm, double minDistSquared) const
{
	Zone* zoneOut = nullptr;
	pOutInMm = trip.B();
	const auto tripLine = trip.Line();

	// Check all zones for borders intersections with mouse direction.
	for (const auto zone : Layout.Zones)
	{
		// exclude zone we are currently in.
		if (current == zone) continue;

		// if new point is within zone lets use it (it may append when allowing zones overlap)
		if (zone->Contains(trip.B()))
		{
			zoneOut = zone;
			minDistSquared = 0;
			LOG_TRACE_1("#");
		}
		else
		{
			//check intersection between the line and the four borders of the target zone
			for (auto p : zone->PhysicalInside().Intersect(tripLine))
			{
				LOG_TRACE_1(zone->Name << p);
				if (p.X()<trip.A().X()) 
				{
					if (trip.B().X() > trip.A().X()) 
					{
						LOG_TRACE_1("<");
						continue;
					}
				}
				else
				{
					if (trip.B().X() < trip.A().X()) 
					{
						LOG_TRACE_1(">");
						continue;
					}
				}

				if (p.Y()<trip.A().Y()) 
				{
					if (trip.B().Y() > trip.A().Y())
					{
						LOG_TRACE_1("/\\");
						continue;
					}
				}
				else
				{
					if (trip.B().Y() < trip.A().Y()) 
					{
						LOG_TRACE_1("\\/");
						continue;
					}
				}

				// calculate distance (squared) to retain the intersection with minimal travel distance
				const auto dist = pow(p.X() - trip.B().X(), 2) + pow(p.Y() - trip.B().Y(), 2);

				if (dist > minDistSquared) {
					LOG_TRACE_1("+");
					continue;
				}
				LOG_TRACE_1("*");
				minDistSquared = dist;
				zoneOut = zone;
				pOutInMm = p;
			}
		}
	}
	return zoneOut;
}

bool MouseEngine::CheckForStopped(const MouseEventArg& e)
{
	ResetClip();

	if(e.Running) return false;

	LOG_TRACE("<engine:hook stopped>");
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
	return true;
}

bool MouseEngine::TryPassBorderCross(const Zone* zone, const geo::Segment<double>& trip)
{
	const auto bounds = zone->PhysicalBounds();

	geo::Point<double> p;
	if (geo::Segment(bounds.TopLeft(), bounds.BottomLeft()).IsIntersecting(trip,p))
	{
		LOG_TRACE_1("left : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE_1("distance : " << distance);
		const auto zoneLink = zone->LeftZones->AtPhysical(p.Y());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.TopRight(), bounds.BottomRight()).IsIntersecting(trip,p))
	{
		LOG_TRACE_1("right : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE_1("distance : " << distance);
		const auto zoneLink = zone->RightZones->AtPhysical(p.Y());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.TopLeft(), bounds.TopRight()).IsIntersecting(trip, p))
	{
		LOG_TRACE_1("top : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE_1("distance : " << distance);
		const auto zoneLink = zone->TopZones->AtPhysical(p.X());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.BottomLeft(), bounds.BottomRight()).IsIntersecting(trip, p))
	{
		LOG_TRACE_1("bottom : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE_1("distance : " << distance);
		const auto zoneLink = zone->BottomZones->AtPhysical(p.X());
		return TryPassBorder(zoneLink, distance);
	}

	return false;
}

/// <summary>
/// Process mouse move using cross algorithm (calculate mouvement direction)
/// </summary>
/// <param name="e">Mouse event</param>
void MouseEngine::OnMouseMoveCross(MouseEventArg& e)
{
	if(CheckForStopped(e)) return;

	if (_oldZone->PixelsBounds().Contains(e.Point))
	{
#ifdef _DEBUG
		if(GetAsyncKeyState(VK_SHIFT) & 0x01) 
		{
			LOG_TRACE_1("no change : " << e.Point);
		}
#endif 

		if(_currentResistanceLink)
		{
			LOG_TRACE_1("< nullptr >");
			_currentResistanceLink = nullptr;
		}

		_oldPoint = e.Point;
		e.Handled = false;
		return;
	}

	const auto pInMmOld = _oldZone->ToPhysical(_oldPoint);
	const auto pInMm = _oldZone->ToPhysical(e.Point);

	//Get line from previous point to current point
	const auto trip = geo::Segment(pInMmOld, pInMm);
	const auto minDistSquared = Layout.MaxTravelDistanceSquared;

	geo::Point<double> pOutInMm;

	if(const auto zoneOut = FindTargetZone(_oldZone, trip, pOutInMm, minDistSquared))
	{
		if (TryPassBorderCross(_oldZone, trip))
		{
			MoveInMm(e, pOutInMm, zoneOut);
			return;
		}
		NoZoneMatches(e);
		return;
	}

	if(Layout.LoopX)
	{
		Zone* zoneOut = nullptr;
		// we are moving left
		if(trip.B().X() < trip.A().X())
			zoneOut = FindTargetZone(nullptr, trip + geo::Point<double>(Layout.Width(),0), pOutInMm, minDistSquared);

		// we are moving right
		else if(trip.B().X() > trip.A().X())
			zoneOut = FindTargetZone(nullptr, trip - geo::Point<double>(Layout.Width(),0), pOutInMm, minDistSquared);

		if(zoneOut)
		{
			if (TryPassBorderCross(_oldZone, trip))
			{
				MoveInMm(e, pOutInMm, zoneOut);
				return;
			}
			NoZoneMatches(e);
			return;
		}
	}

	if(Layout.LoopY)
	{
		Zone* zoneOut = nullptr;

		// we are moving top
		if(trip.B().Y() < trip.A().Y())
			zoneOut = FindTargetZone(nullptr, trip + geo::Point<double>(0,Layout.Height()), pOutInMm, minDistSquared);

		// we are moving bottom
		else if(trip.B().Y() > trip.A().Y())
			zoneOut = FindTargetZone(nullptr, trip - geo::Point<double>(0,Layout.Height()), pOutInMm, minDistSquared);

		if(zoneOut) 
		{
			if (TryPassBorderCross(_oldZone, trip))
			{
				MoveInMm(e, pOutInMm, zoneOut);
				return;
			}
			NoZoneMatches(e);
			return;
		}
	}

	LOG_TRACE_1(" - " << pInMmOld << pInMm << " - ");
	NoZoneMatches(e);
}

/// <summary>
/// Deal with border resistance 
/// (Original idea by Kevin Mills https://github.com/mgfarmer/LittleBigMouse)
/// </summary>
/// <param name="zoneLink"></param>
/// <param name="distance"></param>
/// <returns></returns>
bool MouseEngine::TryPassBorder(const ZoneLink* zoneLink, const double distance)
{
	if((GetAsyncKeyState(VK_CONTROL) & 0x8000) == 0x8000) return true;

	if(zoneLink != _currentResistanceLink)
	{
		LOG_TRACE_1("< != >");

		_currentResistanceLink = zoneLink;
		_borderResistance = zoneLink->BorderResistance;
	}

	LOG_TRACE_1("< -= >" << distance);

	_borderResistance -= distance;

	if (_borderResistance<=0) return true;

	return false;
}

/// <summary>
/// Deal with border resistance 
/// (Original idea by Kevin Mills https://github.com/mgfarmer/LittleBigMouse)
/// </summary>
/// <param name="zoneLink"></param>
/// <param name="distance"></param>
/// <returns></returns>
bool MouseEngine::TryPassBorderPixel(const ZoneLink* zoneLink, const long distance)
{
	if((GetAsyncKeyState(VK_CONTROL) & 0x8000) == 0x8000) return true;

	if(zoneLink != _currentResistanceLink)
	{
		LOG_TRACE_1("< != >");

		_currentResistanceLink = zoneLink;
		_borderResistancePixel = zoneLink->BorderResistancePixel;
	}

	LOG_TRACE_1("< -= >" << distance);

	_borderResistancePixel -= distance;

	if (_borderResistancePixel<=0) return true;

	return false;
}

/// <summary>
/// Process mouse move using strait algorithm (pre-calculated border links)
/// </summary>
/// <param name="e">Mouse event</param>
void MouseEngine::OnMouseMoveStraight(MouseEventArg& e)
{
	if(CheckForStopped(e)) return;

	const auto pIn = e.Point;

	const ZoneLink* zoneLinkOut;
	geo::Point<long> pOut;
	const auto bounds = _oldZone->PixelsBounds();

	// leaving zone by right
	if (long dist; (dist = 1 + pIn.X() - bounds.Right()) >= 0)
	{
		//DebugUnhook.fire();

		zoneLinkOut = _oldZone->RightZones->AtPixel(pIn.Y());
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,dist))
		{
			pOut = { zoneLinkOut->Target->PixelsBounds().Left(),zoneLinkOut->ToTargetPixel(pIn.Y()) };
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by left
	else if ((dist = bounds.Left() - pIn.X()) >= 0)
	{
		zoneLinkOut = _oldZone->LeftZones->AtPixel(pIn.Y());
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,dist))
		{
			pOut = { zoneLinkOut->Target->PixelsBounds().Right() - 1,zoneLinkOut->ToTargetPixel(pIn.Y()) };
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by bottom
	else if ((dist = 1 +  pIn.Y() - bounds.Bottom()) >= 0)
	{
		zoneLinkOut = _oldZone->BottomZones->AtPixel(pIn.X());
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,dist))
		{
			pOut = { zoneLinkOut->ToTargetPixel(pIn.X()), zoneLinkOut->Target->PixelsBounds().Top() };
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by top
	else if ((dist = _oldZone->PixelsBounds().Top() - pIn.Y()) >= 0)
	{
		zoneLinkOut = _oldZone->TopZones->AtPixel(pIn.X());
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,dist))
		{
			pOut = { zoneLinkOut->ToTargetPixel(pIn.X()),zoneLinkOut->Target->PixelsBounds().Bottom() - 1 };
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	else
	{
		// no border crossed reset resistance link.
		if(_currentResistanceLink)
		{
			LOG_TRACE_1("< nullptr >");
			_currentResistanceLink = nullptr;
		}

		_oldPoint = pIn;
		e.Handled = false;
		return;
	}

	Move(e, pOut, zoneLinkOut->Target);
}

void MouseEngine::MoveInMm(MouseEventArg& e, const geo::Point<double>& pOutInMm, Zone* zoneOut)
{
	const auto pOut = zoneOut->ToPixels(pOutInMm);

	Move(e, pOut, zoneOut);
}

void MouseEngine::Move(MouseEventArg& e, const geo::Point<long>& pOut, Zone* zoneOut)
{
	const auto travel = _oldZone->TravelPixels(Layout.MainZones, zoneOut);

	// Keep the zone the cursor actually entered: with duplicated displays several
	// zones share the same pixel space but sit at different physical locations,
	// and the exit must be computed from the physical monitor the cursor came in
	// through (#83, #222). Folding onto Main here lost that memory. Loop copies
	// never reach this point: their link targets are redirected to the real zone
	// before serialization.
	_oldZone = zoneOut;
	_oldPoint = pOut;

	const auto r = zoneOut->PixelsBounds();

	SaveClip();
	auto pos = e.Point;

	for (const auto& rect : travel)
	{

		if (rect.Contains(pos)) continue;

		SetClip(rect);

		pos = GetMouseLocation();

		if (rect.Contains(pOut)) break;
	}

	SetClip(r);
	SetMouseLocation(pOut);

	//_oldClipRect = GetClip();
	//SetMouseLocation(pOut);
	LOG_TRACE_1("moved : " << zoneOut->Name << " at " << pOut);

	e.Handled = true;
}

void MouseEngine::Reset()
{
	LOG_TRACE("<engine:Reset>");
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
	// The zone may not survive a layout reload; drop it so nothing
	// dereferences a stale pointer before OnMouseMoveExtFirst reassigns it.
	_oldZone = nullptr;
}

bool MouseEngine::IsFreelookActive() const
{
	// Signal 1: cursor hidden — game called ShowCursor(FALSE), show-count < 0
	CURSORINFO ci{};
	ci.cbSize = sizeof(CURSORINFO);
	GetCursorInfo(&ci);
	if (!(ci.flags & CURSOR_SHOWING)) return true;

	// Signal 2: cursor clipped to a sub-virtual-desktop rect
	// Skip this check when LBM itself set the clip (_oldClipRect not empty means
	// LBM saved the original clip and is managing cursor confinement itself).
	if (_oldClipRect.IsEmpty())
	{
		RECT clip;
		GetClipCursor(&clip);
		const int vsLeft   = GetSystemMetrics(SM_XVIRTUALSCREEN);
		const int vsTop    = GetSystemMetrics(SM_YVIRTUALSCREEN);
		const int vsRight  = vsLeft + GetSystemMetrics(SM_CXVIRTUALSCREEN);
		const int vsBottom = vsTop  + GetSystemMetrics(SM_CYVIRTUALSCREEN);
		if (clip.left > vsLeft || clip.top > vsTop ||
		    clip.right < vsRight || clip.bottom < vsBottom)
			return true;
	}

	return false;
}

void MouseEngine::OnMouseMove(MouseEventArg& e)
{
	if (_lock.try_lock())
	{
		// IsFreelookActive costs ~2µs (GetCursorInfo + GetClipCursor), too much to
		// pay on every event of a high polling rate mouse. Freelook only matters
		// when LBM is about to act, so gate the check:
		// - steady state: only when the cursor touches the current zone border
		//   (interior moves need pure arithmetic only);
		// - while in freelook: re-check at the configured interval to detect exit.
		bool checkFreelook;

		if (_wasFreelook)
		{
			checkFreelook = GetTickCount64() - _lastFreelookCheck
				>= static_cast<ULONGLONG>(Layout.FreelookCheckIntervalMs);
		}
		else if (_onMouseMoveFunc == &MouseEngine::OnMouseMoveExtFirst || !_oldZone)
		{
			// Tracking not initialized yet: no zone to gate against.
			checkFreelook = true;
		}
		else
		{
			// Matches the widest acting conditions of both algorithms
			// (Strait acts from x <= Left() / x >= Right()-1, Cross from !Contains).
			const auto& bounds = _oldZone->PixelsBounds();
			checkFreelook =
				e.Point.X() <= bounds.Left() || e.Point.X() >= bounds.Right() - 1 ||
				e.Point.Y() <= bounds.Top() || e.Point.Y() >= bounds.Bottom() - 1;
		}

		if (checkFreelook)
		{
			_lastFreelookCheck = GetTickCount64();
			const bool freelook = IsFreelookActive();

			if (freelook != _wasFreelook)
			{
				if (freelook)
				{
					// Entering freelook: restore any clip LBM had set so the game
					// gets a clean cursor environment.
					ResetClip();
					Reset();
					LOG_TRACE("<engine:freelook enter>");
				}
				else
				{
					// Leaving freelook: position tracking restarts at next event.
					LOG_TRACE("<engine:freelook exit>");
				}
				_wasFreelook = freelook;
			}
		}

		if (_wasFreelook)
		{
			e.Handled = false;
		}
		else if (_onMouseMoveFunc)
		{
			(this->*_onMouseMoveFunc)(e);
		}

		_lock.unlock();
	}
	else
	{
		e.Handled = false;
	}
}
