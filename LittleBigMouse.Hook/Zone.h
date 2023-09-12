#pragma once
//#include <map>
#include <string>
#include <unordered_map>

#include "tinyxml2.h"

#include "Point.h"
#include "Rect.h"


class ZoneLink;
class ZonesLayout;


class Zone
{
private:
	std::unordered_map<Zone*, std::vector<geo::Rect<long>>> _travels;
	geo::Rect<long> _pixelsBounds;
	geo::Rect<double> _physicalBounds;

	std::vector<geo::Rect<long>> GetTravelPixels(const std::vector<Zone*>& zones, const Zone* target) const;

public:

   bool operator==(const Zone& rhs) const noexcept
   {
      return this->Name == rhs.Name; 
   }

	int Id;
	std::string DeviceId;
	std::string Name;

	[[nodiscard]] geo::Rect<long> PixelsBounds() const { return _pixelsBounds;}
	[[nodiscard]] geo::Rect<double> PhysicalBounds() const { return _physicalBounds;}

	Zone* Main;
	ZoneLink* LeftZones;
	ZoneLink* TopZones;
	ZoneLink* RightZones;
	ZoneLink* BottomZones;

	[[nodiscard]] bool IsMain() const;

	double Dpi;

	void ComputeDpi();
	void InitZoneLinks(const ZonesLayout* layout) const;

	geo::Point<double> ToPhysical(geo::Point<long> px) const;
	geo::Point<long> ToPixels(geo::Point<double> mm) const;

	geo::Point<long> CenterPixel() const;

	bool Contains(const geo::Point<long>& pixel) const;

	bool Contains(const geo::Point<double>& mm) const;

	geo::Point<long> InsidePixelsBounds(geo::Point<long> px) const;
	geo::Point<double> InsidePhysicalBounds(geo::Point<double> mm) const;

	std::vector<geo::Rect<long>>& TravelPixels(const std::vector<Zone*>& zones, const Zone* target);


	Zone(
		int id,
		std::string deviceId,
		std::string name,
		const geo::Rect<long>& pixelsBounds,
		const geo::Rect<double>& physicalBounds,
            Zone* main = nullptr)
	:Id(id)
	,_pixelsBounds(pixelsBounds)
	,_physicalBounds(physicalBounds)
	,DeviceId(std::move(deviceId))
	,Name(std::move(name))
	,Main(main)
        {
			if(!Main) Main = this;

            const double dpiX = _pixelsBounds.Width() / (_physicalBounds.Width() / 25.4);
            const double dpiY = _pixelsBounds.Height() / (_physicalBounds.Height() / 25.4);

            Dpi = sqrt(dpiX * dpiX + dpiY * dpiY) / sqrt(2);
        }

	~Zone();


	bool HorizontalReachable(const geo::Point<double>& mm) const;
	bool VerticalReachable(const geo::Point<double>& mm) const;
};

