#pragma once
#include "Zone.h"
#include "tinyxml2.h"

class ZonesLayout
{

public:
	bool AdjustPointer;
	bool AdjustSpeed;

	Zone* Containing(const POINT& pixel) const;
	Zone* Containing(const Point<double>& physical) const;

	std::vector<Zone*> Zones;
	std::vector<Zone*> MainZones;

	void Init();
	void Load(tinyxml2::XMLElement* layoutElement);
	void Unload();

	~ZonesLayout()
	{
		Unload();
	}
};

