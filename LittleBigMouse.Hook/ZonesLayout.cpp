#include "ZonesLayout.h"
#include "XmlHelper.h"

Zone* ZonesLayout::Containing(const POINT& pixel) const
{
	for (auto zone : MainZones)
	{
		if(zone->Contains(pixel)) return zone;
	}
	return nullptr;
}

Zone* ZonesLayout::Containing(const Point<double>& physical) const
{
	for (auto zone : MainZones)
	{
		if(zone->Contains(physical)) return zone;
	}
	return nullptr;
}

void ZonesLayout::Init()
{
	for (auto zone : Zones)
	{
		if(zone->IsMain()) MainZones.push_back(zone);
	}
	for (auto zone : Zones)
	{
		zone->Init();
	}
}

void ZonesLayout::Load(tinyxml2::XMLElement* layoutElement)
{
	Unload();

	AdjustPointer = XmlHelper::GetBool(layoutElement,"AdjustPointer");
	AdjustSpeed = XmlHelper::GetBool(layoutElement,"AdjustSpeed");

	auto zonesElement = layoutElement->FirstChildElement("Zones");
	if(zonesElement)
	{
		auto zoneElement = zonesElement->FirstChildElement("Zone");
		while(zoneElement)
		{
			auto zone = Zone::GetNewZone(zoneElement);
			if(zone) Zones.push_back(zone);

			zoneElement = zoneElement->NextSiblingElement("Zone");
		}
	}

	Init();
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

