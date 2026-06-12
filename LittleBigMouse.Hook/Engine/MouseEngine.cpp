#include "MouseEngine.h"

#include "Geometry/Geometry.h"

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

	const auto p = _oldZone->ToPhysical(e.Point);
	LOG_TRACE("NoZoneMatches : " << _oldZone->Name << " at " << e.Point << " (" << p << ")" );

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
	ClearClip();

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

		ClearClip();
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
		const Zone* zoneOut = nullptr;
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
		const Zone* zoneOut = nullptr;

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

	// DisplayLink workaround: suppress immediate bounce-back after zone transition
	if (_suppressNextBounce)
	{
		_suppressNextBounce = false;
		// If cursor was moved by Windows/DisplayLink (not by user), correct it back
		const auto dx = abs(pIn.X() - _intendedPoint.X());
		const auto dy = abs(pIn.Y() - _intendedPoint.Y());
		// Check for either large Y displacement or large X displacement (DisplayLink can move both)
		if ((dx < 5 && dy > 100) || (dy < 5 && dx > 100) || (dx > 100 && dy > 100))
		{
			LOG_TRACE("Suppressing DisplayLink bounce: intended [" << _intendedPoint.X() << "," << _intendedPoint.Y() << "] actual [" << pIn.X() << "," << pIn.Y() << "] - correcting position");
			SetMouseLocation(_intendedPoint);
			_oldPoint = _intendedPoint;
			e.Handled = true;
			return;
		}
	}

	const ZoneLink* zoneLinkOut;
	geo::Point<long> pOut;
	const auto bounds = _oldZone->PixelsBounds();

	// Calculate distances to all borders
	const long distRight = 1 + pIn.X() - bounds.Right();
	const long distLeft = bounds.Left() - pIn.X();
	const long distBottom = 1 + pIn.Y() - bounds.Bottom();
	const long distTop = bounds.Top() - pIn.Y();

	LOG_TRACE("Pos=" << pIn << " Bounds=[" << bounds.Left() << "," << bounds.Top() << " to " << bounds.Right() << "," << bounds.Bottom() << "] Dist: R=" << distRight << " L=" << distLeft << " B=" << distBottom << " T=" << distTop);

	// Determine which border is being crossed (prefer the one with larger distance)
	// This handles corner cases where cursor is exactly on an edge
	// Note: Only consider borders that have been crossed (dist > 0), not just touched (dist == 0)
	long maxDist = 0; // Changed from -1 to 0 so we require dist > 0
	int borderCrossed = -1; // 0=right, 1=left, 2=bottom, 3=top

	if (distRight > maxDist) { maxDist = distRight; borderCrossed = 0; }
	if (distLeft > maxDist) { maxDist = distLeft; borderCrossed = 1; }
	if (distBottom > maxDist) { maxDist = distBottom; borderCrossed = 2; }
	if (distTop > maxDist) { maxDist = distTop; borderCrossed = 3; }

	// leaving zone by right
	if (borderCrossed == 0)
	{
		//DebugUnhook.fire();

		zoneLinkOut = _oldZone->RightZones->AtPixel(pIn.Y());
		LOG_TRACE("RIGHT border: Y=" << pIn.Y() << " -> link target=" << (zoneLinkOut->Target ? zoneLinkOut->Target->Name : "NULL") << " targetId=" << zoneLinkOut->TargetId);
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,distRight))
		{
			// Convert to physical coordinates and back to get proper position
			const auto pInPhysical = _oldZone->ToPhysical(pIn);
			const auto pOutPhysical = geo::Point<double>(zoneLinkOut->Target->PhysicalBounds().Left(), pInPhysical.Y());
			pOut = zoneLinkOut->Target->ToPixels(pOutPhysical);
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by left
	else if (borderCrossed == 1)
	{
		zoneLinkOut = _oldZone->LeftZones->AtPixel(pIn.Y());
		LOG_TRACE("LEFT border: Y=" << pIn.Y() << " -> link target=" << (zoneLinkOut->Target ? zoneLinkOut->Target->Name : "NULL") << " targetId=" << zoneLinkOut->TargetId);
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,distLeft))
		{
			// Convert to physical coordinates and back to get proper position
			const auto pInPhysical = _oldZone->ToPhysical(pIn);
			const auto pOutPhysical = geo::Point<double>(zoneLinkOut->Target->PhysicalBounds().Right(), pInPhysical.Y());
			pOut = zoneLinkOut->Target->ToPixels(pOutPhysical);
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by bottom
	else if (borderCrossed == 2)
	{
		zoneLinkOut = _oldZone->BottomZones->AtPixel(pIn.X());
		LOG_TRACE("BOTTOM border: X=" << pIn.X() << " -> link target=" << (zoneLinkOut->Target ? zoneLinkOut->Target->Name : "NULL") << " targetId=" << zoneLinkOut->TargetId);
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,distBottom))
		{
			// Convert to physical coordinates and back to get proper position
			const auto pInPhysical = _oldZone->ToPhysical(pIn);
			const auto pOutPhysical = geo::Point<double>(pInPhysical.X(), zoneLinkOut->Target->PhysicalBounds().Top());
			pOut = zoneLinkOut->Target->ToPixels(pOutPhysical);
		}
		else
		{
			NoZoneMatches(e);
			return;
		}
	}
	// leaving zone by top
	else if (borderCrossed == 3)
	{
		zoneLinkOut = _oldZone->TopZones->AtPixel(pIn.X());
		LOG_TRACE("TOP border: X=" << pIn.X() << " -> link target=" << (zoneLinkOut->Target ? zoneLinkOut->Target->Name : "NULL") << " targetId=" << zoneLinkOut->TargetId);
		if (zoneLinkOut->Target && TryPassBorderPixel(zoneLinkOut,distTop))
		{
			// Convert to physical coordinates and back to get proper position
			const auto pInPhysical = _oldZone->ToPhysical(pIn);
			const auto pOutPhysical = geo::Point<double>(pInPhysical.X(), zoneLinkOut->Target->PhysicalBounds().Bottom());
			pOut = zoneLinkOut->Target->ToPixels(pOutPhysical);
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

		ClearClip();
		_oldPoint = pIn;
		e.Handled = false;
		return;
	}

	Move(e, pOut, zoneLinkOut->Target);
}

void MouseEngine::MoveInMm(MouseEventArg& e, const geo::Point<double>& pOutInMm, const Zone* zoneOut)
{
	const auto pOut = zoneOut->ToPixels(pOutInMm);

	Move(e, pOut, zoneOut);
}

void MouseEngine::Move(MouseEventArg& e, const geo::Point<long>& pOut, const Zone* zoneOut)
{
	LOG_TRACE("Move from " << _oldZone->Name << " (" << e.Point << ") to " << zoneOut->Name << " at " << pOut);

	const auto travel = _oldZone->TravelPixels(Layout.MainZones, zoneOut);

	_oldZone = zoneOut->Main;
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

	// Clear clip completely after move to avoid cursor being trapped
	ClearClip();

	// Track intended position to suppress DisplayLink bounce-back
	_intendedPoint = pOut;
	_suppressNextBounce = true;

	LOG_TRACE("moved : " << zoneOut->Name << " at " << pOut);

	e.Handled = true;
}

void MouseEngine::Reset()
{
	LOG_TRACE("<engine:Reset>");
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
}

void MouseEngine::OnMouseMove(MouseEventArg& e)
{
	//LOG_TRACE(e.Point);

	if(_lock.try_lock())
	{
		if(_onMouseMoveFunc)
			(this->*_onMouseMoveFunc)(e);

		_lock.unlock();
	}
	else
	{
		e.Handled = false;
	}
}
