#include "pch.h"
#include "ZonesLayout.h"
#include "XmlHelper.h"
#include "ZoneLink.h"
#include "Zone.h"

Zone* ZonesLayout::Containing(const geo::Point<long>& pixel) const
{
	for (const auto zone : MainZones)
	{
		if(zone->Contains(pixel)) return zone;
	}
	return nullptr;
}

Zone* ZonesLayout::Containing(const geo::Point<double>& physical) const
{
	for (const auto zone : MainZones)
	{
		if(zone->Contains(physical)) return zone;
	}
	return nullptr;
}

double ZonesLayout::Width() const
{
	return 40.0 + _right - _left;
}

double ZonesLayout::Height() const
{
	return 40.0 + _bottom - _top;
}

bool IsNearer(const int id, int& idMin, const double distance,  double& min)
{
	//if (distance<0.0) return false;
	if (distance>=min) return false;
	if (distance<min) idMin = INT_MAX;
	if (id>=idMin) return false;
	idMin = id;
	min = distance;
	return true;
}


void AddLocation(std::vector<double>& list, double value)
{
}

void ZonesLayout::Init()
{
	for (auto zone : Zones)
	{
		if(zone->PhysicalBounds().Left()<_left) _left = zone->PhysicalBounds().Left();
		if(zone->PhysicalBounds().Top()<_top) _top = zone->PhysicalBounds().Top();
		if(zone->PhysicalBounds().Right()>_right) _right = zone->PhysicalBounds().Right();
		if(zone->PhysicalBounds().Bottom()>_bottom) _bottom = zone->PhysicalBounds().Bottom();

		if(zone->IsMain()) MainZones.push_back(zone);
		zone->ComputeDpi();
		zone->InitZoneLinks(this);
	}

}

void ZonesLayout::Load(tinyxml2::XMLElement* layoutElement)
{
	Unload();

	MaxTravelDistanceSquared = pow(XmlHelper::GetDouble(layoutElement,"MaxTravelDistance"),2);
	AdjustPointer = XmlHelper::GetBool(layoutElement,"AdjustPointer");
	AdjustSpeed = XmlHelper::GetBool(layoutElement,"AdjustSpeed");
	LoopX = XmlHelper::GetBool(layoutElement,"LoopX");
	LoopY = XmlHelper::GetBool(layoutElement,"LoopY");

	const auto algorithm =  XmlHelper::GetString(layoutElement,"Algorithm");
	if(algorithm=="Cross")
		Algorithm = CornerCrossing;
	else
		Algorithm = Strait;

	const auto priority =  XmlHelper::GetString(layoutElement,"Priority");

	if(priority=="Idle")
		Priority = Idle;
	else if(priority=="Below")
		Priority = Below;
	else if(priority=="Normal")
		Priority = Normal;
	else if(priority=="Above")
		Priority = Above;
	else if(priority=="High")
		Priority = High;
	else if(priority=="Realtime")
		Priority = Realtime;
	else
		Priority = Normal;

	if(const auto zonesElement = layoutElement->FirstChildElement("MainZones"))
	{
		auto zoneElement = zonesElement->FirstChildElement("Zone");
		while(zoneElement)
		{
			if(auto zone = GetNewZone(zoneElement)) Zones.push_back(zone);

			zoneElement = zoneElement->NextSiblingElement("Zone");
		}
	}

	Init();
}

ZoneLink* GetNewZoneLink(tinyxml2::XMLElement* element)
{
	if(element==nullptr) return nullptr;

    ZoneLink* link = nullptr;
    ZoneLink** currentLink = &link;
		auto zoneElement = element->FirstChildElement("ZoneLink");
		while(zoneElement)
		{
            const double from = XmlHelper::GetDouble(zoneElement,"From");
            const double to = XmlHelper::GetDouble(zoneElement,"To");
            const long sourceFromPixel = XmlHelper::GetLong(zoneElement,"SourceFromPixel");
            const long sourceToPixel = XmlHelper::GetLong(zoneElement,"SourceToPixel");
            const long targetFromPixel = XmlHelper::GetLong(zoneElement,"TargetFromPixel");
            const long targetToPixel = XmlHelper::GetLong(zoneElement,"TargetToPixel");
            const long targetId = XmlHelper::GetLong(zoneElement,"TargetId");
			*currentLink = new ZoneLink(from,to,sourceFromPixel,sourceToPixel,targetFromPixel,targetToPixel,targetId);

            currentLink = &((*currentLink)->Next);

			zoneElement = zoneElement->NextSiblingElement("ZoneLink");
		}
    return link;
}


Zone* ZonesLayout::GetNewZone(tinyxml2::XMLElement* element) const
{
	const long id = XmlHelper::GetLong(element,"Id");
	const std::string deviceId = XmlHelper::GetString(element,"DeviceId");
	const std::string name = XmlHelper::GetString(element,"Name");
	const geo::Rect<long> pixelsBounds = XmlHelper::GetRectLong(element,"PixelsBounds");
	const geo::Rect<double> physicalBounds = XmlHelper::GetRectDouble(element,"PhysicalBounds");

	Zone* main = nullptr;

    for (const auto zone : Zones)
	{
        if(pixelsBounds == zone->PixelsBounds())
        {
	        main = zone;
            break;
        }
    }

	const auto zone = new Zone(id,deviceId,name,pixelsBounds,physicalBounds,main);

    zone->LeftZones = GetNewZoneLink(element->FirstChildElement("LeftLinks"));
    zone->TopZones = GetNewZoneLink(element->FirstChildElement("TopLinks"));
    zone->RightZones = GetNewZoneLink(element->FirstChildElement("RightLinks"));
    zone->BottomZones = GetNewZoneLink(element->FirstChildElement("BottomLinks"));

	return zone;
}


void ZonesLayout::Unload()
{
	MainZones.clear();

	while(!Zones.empty())
	{
		delete(Zones.back());
		Zones.pop_back();
	}
}

ZonesLayout::~ZonesLayout()
{
	Unload();
}

