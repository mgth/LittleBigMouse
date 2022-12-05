#include "Zone.h"
#include "tinyxml2.h"
#include "XmlHelper.h"

bool Zone::IsMain() const
{
    return Main == this;
}

void Zone::Init()
{
            const double dpiX = _pixelsBounds.Width() / (_physicalBounds.Width() / 25.4);
            const double dpiY = _pixelsBounds.Height() / (_physicalBounds.Height() / 25.4);

            Dpi = sqrt(dpiX * dpiX + dpiY * dpiY) / sqrt(2);
}

Point<double> Zone::ToPhysical(const Point<long> px) const
{
    auto x = _physicalBounds.Left() + ((static_cast<double>(px.X() - _pixelsBounds.Left())) * _physicalBounds.Width() / static_cast<double>(_pixelsBounds.Width()));
    auto y = _physicalBounds.Top() + ((static_cast<double>(px.Y() - _pixelsBounds.Top())) * _physicalBounds.Height() / static_cast<double>(_pixelsBounds.Height()));

    return {x,y};
}

POINT Zone::ToPixels(const Point<double> mm) const
{
    auto x = _pixelsBounds.Left() + static_cast<long>((mm.X() - _physicalBounds.Left()) * static_cast<double>(_pixelsBounds.Width()) / _physicalBounds.Width());
    auto y = _pixelsBounds.Top() + static_cast<long>((mm.Y() - _physicalBounds.Top()) * static_cast<double>(_pixelsBounds.Height()) / _physicalBounds.Height());

    return {x,y};
}

POINT Zone::CenterPixel() const
{
    auto x = _pixelsBounds.Left() + _pixelsBounds.Right() / 2;
    auto y = _pixelsBounds.Top() + _pixelsBounds.Bottom() / 2;

    return {x,y};
}

bool Zone::Contains(const POINT& pixel) const
{
    if (pixel.x < _pixelsBounds.Left()) return false;
    if (pixel.y < _pixelsBounds.Top()) return false;
    if (pixel.x >= _pixelsBounds.Right()) return false;
    if (pixel.y >= _pixelsBounds.Bottom()) return false;
    return true;
}

bool Zone::Contains(const Point<double>& mm) const
{
    return _physicalBounds.Contains(mm);
}

POINT Zone::InsidePixelsBounds(const POINT px) const
{
    auto x = px.x;
    auto y = px.y;

    if (x < _pixelsBounds.Left()) x = _pixelsBounds.Left();
    else if (x > _pixelsBounds.Right() - 1) x = _pixelsBounds.Right() - 1;

    if (y < _pixelsBounds.Top()) y = _pixelsBounds.Top();
    else if (y > _pixelsBounds.Bottom() - 1) y = _pixelsBounds.Bottom() - 1;

    return {x,y};

}

Point<double> Zone::InsidePhysicalBounds(const Point<double> mm) const
{
    double x = mm.X();
    double y = mm.Y();

    if (x < _physicalBounds.Left()) x = _physicalBounds.Left();
    else if (x > _physicalBounds.Right()) x = _physicalBounds.Right();

    if (y < _physicalBounds.Top()) y = _physicalBounds.Top();
    else if (y > _physicalBounds.Bottom()) y = _physicalBounds.Bottom();

    return {x,y};
}

std::vector<RECT>& Zone::TravelPixels(const std::vector<Zone*>& zones, const Zone* target)
{
	const auto it = _travels.find(target->Main);
    if(it != _travels.end())
	{
        return it->second;
    }

	std::vector<RECT> l = GetTravelPixels(zones, target);

    _travels[target->Main] = l;

    return l;
}

const RECT& ToRECT(const Rect<long>& r)
{
    return RECT{r.Left(),r.Top(),r.Right(),r.Bottom()};
}


std::vector<RECT> Reachable(const RECT& source, const RECT& target) 
{
	const auto left = max(target.left,source.left);
	const auto right = min(target.right,source.right);
	const auto top = max(target.top,source.top);
	const auto bottom = min(target.bottom,source.bottom);

    if(left >= right) 
    {
        if(top >= bottom)
        {
            return {source};
        }

        auto start = RECT{source.left, top, source.right, bottom};
        auto dest  = RECT{target.left, top, target.right, bottom};
        return {start,dest};
    }

	if(top >= bottom) 
	{
        auto start = RECT{left, source.top, right, source.bottom};
        auto dest  = RECT{left, target.top, right, target.bottom};
		return {start,dest};
	}

    auto start = RECT{left, top, right, bottom};
	return {start,start};
}

std::vector<RECT> Travel(const RECT& source, const RECT& target, const std::vector<RECT>& allowed)  
{
	auto reachable = Reachable(source,target);
	if( reachable.size()>1) return reachable;

	if (allowed.empty()) return {};

	for(const auto& next: allowed)
	{
		
		std::vector<RECT> newAllowed;
		std::copy_if (allowed.begin(), allowed.end(), std::back_inserter(newAllowed), 
            [next](const RECT& value) { return 
                value.left != next.left
                && value.top != next.top 
                && value.right != next.right 
                && value.bottom != next.bottom ; 
            
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

std::vector<RECT> Zone::GetTravelPixels(const std::vector<Zone*>& zones, const Zone* target) const
{
	std::vector<RECT> bounds;
	for(const auto& zone: zones)
	{
        if(zone->IsMain()) bounds.push_back(ToRECT(zone->PixelsBounds()));
    }
	return Travel(ToRECT(_pixelsBounds),ToRECT(target->PixelsBounds()),bounds);
}

Zone* Zone::GetNewZone(tinyxml2::XMLElement* zoneElement)
{
	std::string deviceId = XmlHelper::GetString(zoneElement,"DeviceId");
	std::string name = XmlHelper::GetString(zoneElement,"Name");
	Rect<long> pixelsBounds = XmlHelper::GetRectLong(zoneElement,"PixelsBounds");
	Rect<double> physicalBounds = XmlHelper::GetRectDouble(zoneElement,"PhysicalBounds");

	auto zone = new Zone(deviceId,name,pixelsBounds,physicalBounds);
	return zone;
}
