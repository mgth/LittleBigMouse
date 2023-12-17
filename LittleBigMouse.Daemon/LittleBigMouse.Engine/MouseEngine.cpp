#include "pch.h"
#include "MouseEngine.h"
#include <iostream>
#include <windows.h>

#include "Geometry.h"
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
	std::cout << "NoZoneMatches : " << _oldZone->Name << e.Point << p << "\n";
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
#ifdef _DEBUG_
			std::cout << "#";
#endif
		}
		else
		{
			//check intersection between the line and the four borders of the target zone
			for (auto p : zone->PhysicalInside().Intersect(tripLine))
			{
#ifdef _DEBUG_
				std::cout << zone->Name << p;
#endif
				if (p.X()<trip.A().X()) 
				{
					if (trip.B().X() > trip.A().X()) 
					{

#ifdef _DEBUG_
						std::cout << "<\n";
#endif
						continue;
					}
				}
				else
				{
					if (trip.B().X() < trip.A().X()) 
					{
#ifdef _DEBUG_
						std::cout << ">\n";
#endif
						continue;
					}
				}

				if (p.Y()<trip.A().Y()) 
				{
					if (trip.B().Y() > trip.A().Y())
					{
#ifdef _DEBUG_
						std::cout << "/\\\n";
#endif
						continue;
					}
				}
				else
				{
					if (trip.B().Y() < trip.A().Y()) 
					{
#ifdef _DEBUG_
						std::cout << "\\/\n";
#endif
						continue;
					}
				}

				// calculate distance (squared) to retain the intersection with minimal travel distance
				const auto dist = pow(p.X() - trip.B().X(), 2) + pow(p.Y() - trip.B().Y(), 2);

				if (dist > minDistSquared) {
#ifdef _DEBUG_
					std::cout << "+\n";
#endif
					continue;
				}

#ifdef _DEBUG_
				std::cout << "*\n";
#endif
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
	#ifdef _DEBUG
		std::cout << "<engine:nohook>" << std::endl;
	#endif
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
	return true;
}

void MouseEngine::OnMouseMoveCross(MouseEventArg& e)
{
	ResetClip();
	if(CheckForStopped(e)) return;

	if (_oldZone->PixelsBounds().Contains(e.Point))
	{
#ifdef _DEBUG_
		std::cout << "no change : " << pIn << "\n";
#endif

		_oldPoint = e.Point;
		e.Handled = false;
		return;
	}

	const auto pInMmOld = _oldZone->ToPhysical(_oldPoint);
	const auto pInMm = _oldZone->ToPhysical(e.Point);

	//Get line from previous point to current point
	const auto trip = geo::Segment<double>(pInMmOld, pInMm);
	const auto minDistSquared = Layout.MaxTravelDistanceSquared;

	geo::Point<double> pOutInMm;

	if(const auto zoneOut = FindTargetZone(_oldZone, trip, pOutInMm, minDistSquared))
	{
		MoveInMm(e, pOutInMm, zoneOut);
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
			MoveInMm(e, pOutInMm, zoneOut);
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
			MoveInMm(e, pOutInMm, zoneOut);
			return;
		}
	}
#ifdef _DEBUG_
	std::cout << " - " << pInMmOld << pInMm << " - ";
#endif
	NoZoneMatches(e);
}

void MouseEngine::OnMouseMoveStraight(MouseEventArg& e)
{
	ResetClip();
	if(CheckForStopped(e)) return;

	const auto pIn = e.Point;

	const ZoneLink* zoneOut;
	geo::Point<long> pOut;
	const auto bounds = _oldZone->PixelsBounds();
	// leaving zone by right
	if (pIn.X() >= bounds.Right())
	{
		zoneOut = _oldZone->RightZones->AtPixel(pIn.Y());
		if (zoneOut->Target)
		{
			pOut = { zoneOut->Target->PixelsBounds().Left(),zoneOut->ToTargetPixel(pIn.Y()) };
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
			pOut = { zoneOut->Target->PixelsBounds().Right() - 1,zoneOut->ToTargetPixel(pIn.Y()) };
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
		zoneOut = _oldZone->BottomZones->AtPixel(pIn.X());
		if (zoneOut->Target)
		{
			pOut = { zoneOut->ToTargetPixel(pIn.X()), zoneOut->Target->PixelsBounds().Top() };
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
			pOut = { zoneOut->ToTargetPixel(pIn.X()),zoneOut->Target->PixelsBounds().Bottom() - 1 };
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

	Move(e, pOut, zoneOut->Target);
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

#ifdef _DEBUG
	std::cout << "moved : " << zoneOut->Name << " at " << pOut << "\n";
#endif

	e.Handled = true;
}

void MouseEngine::Reset()
{
	_onMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
}

void MouseEngine::OnMouseMove(MouseEventArg& e)
{
#ifdef _DEBUG_
	std::cout << e.Point << "\n";
#endif

	if(_lock.try_lock())
	{
		(this->*_onMouseMoveFunc)(e);
		_lock.unlock();
	}
	else
	{
		e.Handled = false;
	}
	//_lock.lock();
	//(this->*OnMouseMoveFunc)(e);
	//_lock.unlock();

	//if(e.Timing())
	//{
	//	std::cout << e.GetDuration() << "\n";
	//}
}
