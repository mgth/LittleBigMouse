#pragma once
#include "Framework.h"

#include <vector>

#include "Geometry/Point.h"
#include "Xml/tinyxml2.h"
#include "Priority.h"

class Zone;

enum Algorithm {Strait,CornerCrossing};

class ZonesLayout
{
	Zone* GetNewZone(tinyxml2::XMLElement* xmlElement) const;

	double _left = 0.0;
	double _top = 0.0;
	double _right = 0.0;
	double _bottom = 0.0;

public:
	double MaxTravelDistanceSquared = pow(200.0,2.0);

	bool AdjustPointer = false;
	bool AdjustSpeed = false;

	Algorithm Algorithm = Strait;
	Priority PriorityUnhooked = Above;
	Priority Priority = Normal;

	bool LoopX = false;
	bool LoopY = false;

	[[nodiscard]] Zone* Containing(const geo::Point<long>& pixel) const;
	[[nodiscard]] Zone* Containing(const geo::Point<double>& physical) const;

	[[nodiscard]] double Width() const;
	[[nodiscard]] double Height() const;

	std::vector<Zone*> Zones;
	std::vector<Zone*> MainZones;

	void Init();

	void Load(tinyxml2::XMLElement* layoutElement);

	void Unload();

	~ZonesLayout();
};

