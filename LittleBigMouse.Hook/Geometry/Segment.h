#pragma once
#include "Framework.h"

#include <vector>

#include "Point.h"
#include "Line.h"

namespace geo
{

	template<class T>
	class Segment
	{
	private:
		Point<T> _a;
		Point<T> _b;

	public:
		Segment(Point<T> a, Point<T> b) :_a(a), _b(b) {}
		Point<T> A() const { return _a; }
		Point<T> B() const { return _b; }
		Line<T> Line() const
		{
			if (_a.X() == _b.X())
			{
				return { _a.X() };
			}

			const double slope = static_cast<double>(_a.Y() - _b.Y()) / static_cast<double>(_a.X() - _b.X());
			const double origin = _a.Y() - slope * _a.X();

			return { slope, origin };
		}

		[[nodiscard]] double Size() const
		{
			return sqrt(SizeSquared());
		}

		T SizeSquared() const
		{
			auto w = _b.X() - _a.X();
			auto h = _b.Y() - _a.Y();

			return (w * w) + (h * h);
		}

		[[nodiscard]] Point<T> Intersect(const geo::Line<T>& line) const
		{
			constexpr double epsilon = 0.001;
			Point<T> p;
			if (Line().IsIntersecting(line, &p))
			{
				if (p.X() < min(_a.X(), _b.X()) - epsilon) return Point<double>::Empty();
				if (p.Y() < min(_a.Y(), _b.Y()) - epsilon) return Point<double>::Empty();
				if (p.X() > max(_a.X(), _b.X()) + epsilon) return Point<double>::Empty();
				if (p.Y() > max(_a.Y(), _b.Y()) + epsilon) return Point<double>::Empty();
			}
			return p;
		}

		static bool OutSide(T n, T p1, T p2)
		{
			constexpr double epsilon = 0.001;

			if (n<p1-epsilon && n<p2-epsilon) return true;
			if (n>p1+epsilon && n>p2+epsilon) return true;
			return false;
		}

		bool IsIntersecting(const geo::Line<T>& line, geo::Point<T>& point) const
		{
			geo::Point<T> p;
			if(Line().IsIntersecting(line,p))
			{
				if(OutSide(p.X(),_a.X(),_b.X()) || OutSide(p.Y(),_a.Y(),_b.Y())) 
				{
					point = Point<T>::Empty();
					return false;
				}

				point = p;
				return true;
			}
			point = Point<T>::Empty();
			return false;
		}

		[[nodiscard]] std::vector<Point<T>> IntersectList(const geo::Line<T>& line) const
		{
			std::vector<Point<T>> result;
			if (Point<T> p; IsIntersecting(line,p))
			{
				result.push_back(p);
			}
			return result;
		}

		[[nodiscard]] bool IsIntersecting(const Segment& s, geo::Point<T>& point) const
		{
			constexpr double epsilon = 0.001;

			auto p = Point<T>::Empty();
			if (s.IsIntersecting(Line(), p))
			{
				if (OutSide(p.X(), _a.X(), _b.X()) || OutSide(p.Y(), _a.Y(), _b.Y())) 
				{
					point = Point<T>::Empty();
					return false;
				}
				point = p;
				return true;
			}
			point = Point<T>::Empty();
			return false;
		}

		[[nodiscard]] Point<T> Intersect(const Segment& s) const
		{
			constexpr double epsilon = 0.001;

			Point<T> p;
			if (s.IsIntersecting(Line(), p)) 
			{
				if (p.X() < min(_a.X(), _b.X()) - epsilon) return Point<double>::Empty();
				if (p.Y() < min(_a.Y(), _b.Y()) - epsilon) return Point<double>::Empty();
				if (p.X() > max(_a.X(), _b.X()) + epsilon) return Point<double>::Empty();
				if (p.Y() > max(_a.Y(), _b.Y()) + epsilon) return Point<double>::Empty();
			}
			return p;
		}

		Segment<T> operator+(const Point<T>& p) const { return { _a + p, _b + p }; }
		Segment<T> operator-(const Point<T>& p) const { return { _a - p, _b - p }; }

	};

}
