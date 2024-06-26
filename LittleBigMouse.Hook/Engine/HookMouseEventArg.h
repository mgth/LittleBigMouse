#pragma once
#include "Framework.h"

#include <chrono>
#include "Geometry/Point.h"

class MouseEventArg
{

public:
	explicit MouseEventArg(const geo::Point<long> point):Point(point){}

	geo::Point<long> Point = geo::Point<long>();
	bool Handled = false;
	bool Running = true;

	[[nodiscard]] bool Timing() const {return _timing;}
	auto StartTiming() { return _timingStart = _timingEnd = GetTime();}
	auto EndTiming()
	{
		_timing=true;
		return _timingEnd = GetTime();
	}

	[[nodiscard]] long long GetDuration() const { return std::chrono::duration_cast<std::chrono::nanoseconds>(_timingEnd - _timingStart).count(); }

private:
	std::chrono::time_point<std::chrono::steady_clock> _timingStart;
	std::chrono::time_point<std::chrono::steady_clock>  _timingEnd;
	bool _timing = false;
	static std::chrono::time_point<std::chrono::steady_clock> GetTime() { return std::chrono::high_resolution_clock::now(); }
};

