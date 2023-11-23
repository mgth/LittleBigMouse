#include "pch.h"
#include "zone.h"

#include <algorithm>
#include <iterator>

#include "tinyxml2.h"
#include "XmlHelper.h"
#include "ZoneLink.h"
#include "ZonesLayout.h"

bool Zone::IsMain() const
{
    return Main == this;
}

void Zone::ComputeDpi()
{
	const double dpiX = _pixelsBounds.Width() / (_physicalBounds.Width() / 25.4);
	const double dpiY = _pixelsBounds.Height() / (_physicalBounds.Height() / 25.4);

	Dpi = sqrt(dpiX * dpiX + dpiY * dpiY) / sqrt(2);
}

geo::Point<double> Zone::ToPhysical(const geo::Point<long> px) const
{
    auto x = _physicalBounds.Left() + ((0.5 + static_cast<double>(px.X() - _pixelsBounds.Left())) * _physicalBounds.Width() / static_cast<double>(_pixelsBounds.Width()));
    auto y = _physicalBounds.Top() + ((0.5 + static_cast<double>(px.Y() - _pixelsBounds.Top())) * _physicalBounds.Height() / static_cast<double>(_pixelsBounds.Height()));

    return {x,y};
}

geo::Point<long> Zone::ToPixels(const geo::Point<double> mm) const
{
	const auto x = _pixelsBounds.Left() + static_cast<long>(((mm.X() - _physicalBounds.Left()) * static_cast<double>(_pixelsBounds.Width()) / _physicalBounds.Width()));
	const auto y = _pixelsBounds.Top() + static_cast<long>(((mm.Y() - _physicalBounds.Top()) * static_cast<double>(_pixelsBounds.Height()) / _physicalBounds.Height()));

    return {x,y};
}

geo::Point<long> Zone::CenterPixel() const
{
    auto x = _pixelsBounds.Left() + _pixelsBounds.Right() / 2;
    auto y = _pixelsBounds.Top() + _pixelsBounds.Bottom() / 2;

    return {x,y};
}

bool Zone::Contains(const geo::Point<long>& pixel) const
{
    if (pixel.X() < _pixelsBounds.Left()) return false;
    if (pixel.Y() < _pixelsBounds.Top()) return false;
    if (pixel.X() >= _pixelsBounds.Right()) return false;
    if (pixel.Y() >= _pixelsBounds.Bottom()) return false;
    return true;
}

bool Zone::Contains(const geo::Point<double>& mm) const
{
    return _physicalBounds.Contains(mm);
}

geo::Point<long> Zone::InsidePixelsBounds(const geo::Point<long> px) const
{
    auto x = px.X();
    auto y = px.X();

    if (x < _pixelsBounds.Left()) x = _pixelsBounds.Left();
    else if (x > _pixelsBounds.Right() - 1) x = _pixelsBounds.Right() - 1;

    if (y < _pixelsBounds.Top()) y = _pixelsBounds.Top();
    else if (y > _pixelsBounds.Bottom() - 1) y = _pixelsBounds.Bottom() - 1;

    return {x,y};

}

geo::Point<double> Zone::InsidePhysicalBounds(const geo::Point<double> mm) const
{
    double x = mm.X();
    double y = mm.Y();

    if (x < _physicalBounds.Left()) x = _physicalBounds.Left();
    else if (x > _physicalBounds.Right()) x = _physicalBounds.Right();

    if (y < _physicalBounds.Top()) y = _physicalBounds.Top();
    else if (y > _physicalBounds.Bottom()) y = _physicalBounds.Bottom();

    return {x,y};
}

std::vector<geo::Rect<long>>& Zone::TravelPixels(const std::vector<Zone*>& zones, const Zone* target)
{
	const auto it = _travels.find(target->Main);
    if(it != _travels.end())
	{
        return it->second;
    }

	const std::vector<geo::Rect<long>> l = GetTravelPixels(zones, target);

    return _travels[target->Main] = l;
}

Zone::Zone(int id, std::string deviceId, std::string name, const geo::Rect<long>& pixelsBounds,
	const geo::Rect<double>& physicalBounds, Zone* main):Id(id)
	                                                     ,_pixelsBounds(pixelsBounds)
	                                                     ,_physicalBounds(physicalBounds)
	                                                     ,_physicalInside(physicalBounds)
	                                                     ,DeviceId(std::move(deviceId))
	                                                     ,Name(std::move(name))
	                                                     ,Main(main)
{
	if(!Main) Main = this;

	const double dpiX = _pixelsBounds.Width() / (_physicalBounds.Width() / 25.4);
	const double dpiY = _pixelsBounds.Height() / (_physicalBounds.Height() / 25.4);

	Dpi = sqrt(dpiX * dpiX + dpiY * dpiY) / sqrt(2);

	double pixelWidth = _physicalBounds.Width() / (double)_pixelsBounds.Width();
	double pixelHeight = _physicalBounds.Height() / (double)_pixelsBounds.Height();

	_physicalInside = geo::Rect<double> (
		_physicalBounds.Left()+pixelWidth/2,
		_physicalBounds.Top()+pixelHeight/2,
		_physicalBounds.Width()-pixelWidth,
		_physicalBounds.Height()-pixelHeight);

}


std::vector<geo::Rect<long>> Reachable(const geo::Rect<long>& source, const geo::Rect<long>& target) 
{
	const auto left = max(target.Left(),source.Left());
	const auto right = min(target.Right(),source.Right());
	const auto top = max(target.Top(),source.Top());
	const auto bottom = min(target.Bottom(),source.Bottom());

    if(left >= right) 
    {
        if(top >= bottom)
        {
            return {source};
        }

        auto start = geo::Rect<long>(source.Left(), top, source.Width(), bottom - top);
        auto dest  = geo::Rect<long>(target.Left(), top, target.Width(), bottom - top);
        return {start,dest};
    }

	if(top >= bottom) 
	{
        auto start = geo::Rect<long>(left, source.Top(), right, source.Height());
        auto dest  = geo::Rect<long>(left, target.Top(), right - left, target.Height());
		return {start,dest};
	}

    auto start = geo::Rect<long>(left, top, right-left, bottom-top);
	return {start,start};
}

std::vector<geo::Rect<long>> Travel(const geo::Rect<long>& source, const geo::Rect<long>& target, const std::vector<geo::Rect<long>>& allowed)  
{
	auto reachable = Reachable(source,target);
	if( reachable.size()>1) return reachable;

	if (allowed.empty()) return {};

	for(const auto& next: allowed)
	{
		
		std::vector<geo::Rect<long>> newAllowed;
		std::copy_if (allowed.begin(), allowed.end(), std::back_inserter(newAllowed), 
            [next](const geo::Rect<long>& value) { return 
		                      value.Left() != next.Left()
			                      && value.Top() != next.Top() 
			                      && value.Right() != next.Right() 
			                      && value.Bottom() != next.Bottom() ; 
		 });

		auto tail = Travel(next,target,newAllowed);
		if(tail.empty()) continue;

		auto travel = Travel(source,next,newAllowed);
		if(!travel.empty()) {
			travel.insert(std::end(travel), std::begin(tail), std::end(tail));
			return travel;
		}
	}

	return {};
}

std::vector<geo::Rect<long>> Zone::GetTravelPixels(const std::vector<Zone*>& zones, const Zone* target) const
{
	std::vector<geo::Rect<long>> bounds;
	for(const auto& zone: zones)
	{
        if(zone->IsMain()) bounds.push_back(zone->PixelsBounds());
    }
	return Travel(_pixelsBounds, target->PixelsBounds(), bounds);
}


void _InitZoneLinks(const ZonesLayout* layout, ZoneLink* link)
{
    while(link)
    {
		const auto id = link->TargetId;
	    for (const auto zone : layout->Zones)
		{
	        if(id == zone->Id)
	        {
		        link->Target = zone;
	            break;
	        }
	    }
	    link = link->Next;
    }
}


void Zone::InitZoneLinks(const ZonesLayout* layout) const
{
	_InitZoneLinks(layout,LeftZones);
	_InitZoneLinks(layout,TopZones);
	_InitZoneLinks(layout,RightZones);
	_InitZoneLinks(layout,BottomZones);
}


Zone::~Zone()
{
	delete LeftZones;
	delete TopZones;
	delete RightZones;
	delete BottomZones;
}

bool Zone::HorizontalReachable(const geo::Point<double>& mm) const
{
    const auto bounds = PhysicalBounds();
    const auto y = mm.Y();
	return bounds.Top() <= y && bounds.Bottom() >= y;
}

bool Zone::VerticalReachable(const geo::Point<double>& mm) const
{
    const auto bounds = PhysicalBounds();
    const auto x = mm.X();
	return bounds.Left() <= x && bounds.Right() >= x;
}
