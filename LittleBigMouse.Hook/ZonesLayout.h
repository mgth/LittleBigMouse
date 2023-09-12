#pragma once
#include <vector>

#include "Point.h"
#include "tinyxml2.h"

class Zone;

enum Algorithm {Strait,CornerCrossing};


class ZonesLayout
{
	Zone* GetNewZone(tinyxml2::XMLElement* xml_element) const;

public:
	bool AdjustPointer;
	bool AdjustSpeed;
	Algorithm Algorithm;

	Zone* Containing(const geo::Point<long>& pixel) const;
	Zone* Containing(const geo::Point<double>& physical) const;

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

