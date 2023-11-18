#pragma once
#include <chrono>

#include "Point.h"

class MouseEventArg
{

public:
	MouseEventArg(const geo::Point<long> point):Point(point){}

	geo::Point<long> Point = geo::Point<long>();
	bool Handled = false;
	bool Running = true;

	bool Timing() const {return _timing;}
	auto StartTiming() { return _timingStart = _timingEnd = getTime();}
	auto EndTiming()
	{
		_timing=true;
		return _timingEnd = getTime();
	}

	long long GetDuration() const { return std::chrono::duration_cast<std::chrono::nanoseconds>(_timingEnd - _timingStart).count(); }

private:
	std::chrono::time_point<std::chrono::steady_clock> _timingStart;
	std::chrono::time_point<std::chrono::steady_clock>  _timingEnd;
	bool _timing = false;
	static std::chrono::time_point<std::chrono::steady_clock> getTime() { return std::chrono::high_resolution_clock::now(); }
};

