#pragma once
#include <vector>

#include "Point.h"
#include "Priority.h"
#include "tinyxml2.h"

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
	bool AdjustPointer;
	bool AdjustSpeed;

	Algorithm Algorithm;
	Priority Priority;

	bool LoopX;
	bool LoopY;

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

