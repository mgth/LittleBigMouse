#pragma once

#include <iostream>
#include <vector>

#include "Point.h"

namespace geo
{
	template<class T>
	class Line
	{
		// Y = _slope * X + _origin
		double _slope;

		// y for x==0
		T _origin;

	public:
		Line(const double slope, T y_for_x0) :_slope(slope), _origin(y_for_x0)
		{
		}

		bool IsVertical() const
		{
			return !(_slope < DBL_MAX);
		}

		// Specific case of vertical line where slope is infinity
		Line(T x) :_slope(DBL_MAX), _origin(x)
		{
		}

		double Slope() const { return _slope; }

		T XatY0() const
		{
			if (IsVertical()) return _origin;
			return _origin / _slope;
		}

		T YatX0() const
		{
			if (IsVertical()) return 0.0;
			return _origin;
		}

		T X(T y) const
		{
			if(_slope == 0.0) return 0.0; // Error
			if(IsVertical()) return _origin;
			return _origin - y / _slope;
		}

		T Y(T x) const
		{
			if(_slope == 0.0) return _origin;
			if(IsVertical()) return 0.0; // Error
			return _slope * x + _origin;
		}

		Point<T> Origin() const
		{
			return { 0, YatX0() };
		}

		bool IsIntersecting(const Line<T>& l, Point<T>& p) const
		{
			T x;
			T y;

			// Lines are parallel
			if (_slope == l._slope)
			{
				//lines are sames, just return origin point.
				if (_origin == l._origin) { p = Origin(); return true; }

				//no intersection   
				return false;
			}

			if (IsVertical())
			{
				x = _origin;
				y =  l.Y(x);
			}
			else
			{
				if (l.IsVertical())
				{
					x = l._origin;
					y = Y(x);
				}
				else
				{
					x = (_origin - l._origin) / (l._slope - _slope);
					y = Y(x);
				}
			}

			p = { x, y };
			return true;
		}


		/*
			std::vector<Point<double>> Intersect(const Segment& s) const;

			std::vector<Point<double>> Intersect(const Rect<double>& rect) const;
		*/
	};
}

