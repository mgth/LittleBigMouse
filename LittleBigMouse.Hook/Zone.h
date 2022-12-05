#pragma once
//#include <map>
#include <string>
#include <unordered_map>

#include "tinyxml2.h"

#include "Point.h"
#include "Rect.h"

class Zone
{
private:
	std::unordered_map<Zone*, std::vector<RECT>> _travels;
	Rect<long> _pixelsBounds;
	Rect<double> _physicalBounds;

	std::vector<RECT> GetTravelPixels(const std::vector<Zone*>& zones, const Zone* target) const;

public:

   bool operator==(const Zone& rhs) const noexcept
   {
      // logic here
      return this->Name == rhs.Name; // for example
   }

	std::string DeviceId;
	std::string Name;

	[[nodiscard]] Rect<long> PixelsBounds() const { return _pixelsBounds;}
	[[nodiscard]] Rect<double> PhysicalBounds() const { return _physicalBounds;}

	Zone* Main;

	[[nodiscard]] bool IsMain() const;

	double Dpi;

	void Init();

	Point<double> ToPhysical(Point<long> px) const;
	POINT ToPixels(Point<double> mm) const;

	POINT CenterPixel() const;

	bool Contains(const POINT& pixel) const;

	bool Contains(const Point<double>& mm) const;

	POINT InsidePixelsBounds(POINT px) const;
	Point<double> InsidePhysicalBounds(Point<double> mm) const;

	std::vector<RECT>& TravelPixels(const std::vector<Zone*>& zones, const Zone* target);


	Zone(
		std::string deviceId,
		std::string name,
		const Rect<long>& pixelsBounds,
		const Rect<double>& physicalBounds,
            Zone* main = nullptr)
	:_pixelsBounds(pixelsBounds)
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

	~Zone()
	{
	}
	
	static Zone* GetNewZone(tinyxml2::XMLElement* zoneElement);
};

