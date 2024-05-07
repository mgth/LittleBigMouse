#include "MouseEngine.h"

#include "Geometry/Geometry.h"

#include "MouseHelper.h"
#include "ZoneLink.h"
#include "Zone.h"

void MouseEngine::OnMouseMoveExtFirst(MouseEventArg& e)
{
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
	_oldClipRect = GetClip();
}

void MouseEngine::ResetClip()
{
	if (!_oldClipRect.IsEmpty())
	{
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
	LOG_TRACE("NoZoneMatches : " << _oldZone->Name << e.Point << p );
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
	if(e.Running) return false;
	LOG_TRACE("<engine:no hook>");
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
	return true;
}

bool MouseEngine::TryPassBorderCross(const Zone* zone, const geo::Segment<double>& trip)
{
	const auto bounds = zone->PhysicalBounds();

	geo::Point<double> p;
	if (geo::Segment(bounds.TopLeft(), bounds.BottomLeft()).IsIntersecting(trip,p))
	{
		LOG_TRACE("left : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE("distance : " << distance);
		const auto zoneLink = zone->LeftZones->AtPhysical(p.Y());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.TopRight(), bounds.BottomRight()).IsIntersecting(trip,p))
	{
		LOG_TRACE("right : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE("distance : " << distance);
		const auto zoneLink = zone->RightZones->AtPhysical(p.Y());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.TopLeft(), bounds.TopRight()).IsIntersecting(trip, p))
	{
		LOG_TRACE("top : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE("distance : " << distance);
		const auto zoneLink = zone->TopZones->AtPhysical(p.X());
		return TryPassBorder(zoneLink, distance);
	}

	if (geo::Segment(bounds.BottomLeft(), bounds.BottomRight()).IsIntersecting(trip, p))
	{
		LOG_TRACE("bottom : " << p);
		const auto distance = geo::Segment(p,trip.B()).Size();
		LOG_TRACE("distance : " << distance);
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
	ResetClip();
	if(CheckForStopped(e)) return;

	if (_oldZone->PixelsBounds().Contains(e.Point))
	{
#ifdef _DEBUG
		if(GetAsyncKeyState(VK_SHIFT) & 0x01) 
		{
			LOG_TRACE("no change : " << e.Point);
		}
#endif 

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
	LOG_TRACE(" - " << pInMmOld << pInMm << " - ");
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
		LOG_TRACE("< != >");

		_currentResistanceLink = zoneLink;
		_borderResistance = zoneLink->BorderResistance;
	}

	LOG_TRACE("< -= >" << distance);

	_borderResistance -= distance;

	if (_borderResistance<=0) return true;

	return false;
}

/// <summary>
/// Process mouse move using strait algorithm (pre-calculated border links)
/// </summary>
/// <param name="e">Mouse event</param>
void MouseEngine::OnMouseMoveStraight(MouseEventArg& e)
{

	ResetClip();
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
		if (zoneLinkOut->Target && TryPassBorder(zoneLinkOut,dist))
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
		if (zoneLinkOut->Target && TryPassBorder(zoneLinkOut,dist))
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
		if (zoneLinkOut->Target && TryPassBorder(zoneLinkOut,dist))
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
		if (zoneLinkOut->Target && TryPassBorder(zoneLinkOut,dist))
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
			LOG_TRACE("< nullptr >");
			_currentResistanceLink = nullptr;
		}

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

	//_oldClipRect = GetClip();
	//SetMouseLocation(pOut);
	LOG_TRACE("moved : " << zoneOut->Name << " at " << pOut);

	e.Handled = true;
}

void MouseEngine::Reset()
{
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
