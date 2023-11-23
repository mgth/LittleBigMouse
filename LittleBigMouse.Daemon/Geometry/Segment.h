#pragma once

#include <iostream>
#include <vector>

#include "Point.h"
#include "Line.h"

// #define DEBUG 1 

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

		double Size() const
		{
			return sqrt(SizeSquared());
		}

		T SizeSquared() const
		{
			auto w = _b.X() - _a.X();
			auto h = _b.Y() - _a.Y();

			return (w * w) + (h * h);
		}

		Point<T> Intersect(const geo::Line<T>& l) const
		{
			constexpr double epsilon = 0.001;

			Point<T> p = Line().Intersect(l);
			if (p.IsEmpty()) return p;

			if (p.X() < min(_a.X(), _b.X()) - epsilon) return Point<double>::Empty();
			if (p.Y() < min(_a.Y(), _b.Y()) - epsilon) return Point<double>::Empty();
			if (p.X() > max(_a.X(), _b.X()) + epsilon) return Point<double>::Empty();
			if (p.Y() > max(_a.Y(), _b.Y()) + epsilon) return Point<double>::Empty();

			return p;
		}

		static bool OutSide(T n, T p1, T p2)
		{
			constexpr double epsilon = 0.001;

			if (n<p1-epsilon && n<p2-epsilon) return true;
			if (n>p1+epsilon && n>p2+epsilon) return true;
			return false;
		}

		bool IsIntersecting(const geo::Line<T>& l, Point<T>& p) const
		{
			Point<T> p1;
				#ifdef _DEBUG_
					std::cout << _a.X() << " , " <<  _a.Y()  << "\n";
					std::cout << _b.X() << " , " <<  _b.Y()  << "\n";
				#endif

			if(Line().IsIntersecting(l,p1))
			{
				#ifdef _DEBUG_
					std::cout << " >> " << p1.X() << " , " << p1.Y()  << "\n";
				#endif

				if(OutSide(p1.X(),_a.X(),_b.X())) return false;
				if(OutSide(p1.Y(),_a.Y(),_b.Y())) return false;

				p = p1;
				return true;
			}
			return false;
		}

		std::vector<Point<T>> IntersectList(const geo::Line<T>& l) const
		{
			std::vector<Point<T>> result;
			const Point<T> p = Intersect(l);
			if (!p.IsEmpty())
			{
				result.push_back(p);
			}
			return result;
		}

		//bool IsIntersecting(const Segment& s, Point<T>& p)
		//{
		//	constexpr double epsilon = 0.001;

		//	p = Line().Intersect(s.Line());

		//	if (p.IsEmpty()) return false;

		//	if (p.X() < min(_a.X(), _b.X()) - epsilon) return false;
		//	if (p.Y() < min(_a.Y(), _b.Y()) - epsilon) return false;
		//	if (p.X() > max(_a.X(), _b.X()) + epsilon) return false;
		//	if (p.Y() > max(_a.Y(), _b.Y()) + epsilon) return false;

		//	return true;
		//}

		Point<T> Intersect(const Segment& s) const
		{
			constexpr double epsilon = 0.001;

			auto p = Line().Intersect(s.Line());

			if (p.IsEmpty()) return p;

			if (p.X() < min(_a.X(), _b.X()) - epsilon) return Point<double>::Empty();
			if (p.Y() < min(_a.Y(), _b.Y()) - epsilon) return Point<double>::Empty();
			if (p.X() > max(_a.X(), _b.X()) + epsilon) return Point<double>::Empty();
			if (p.Y() > max(_a.Y(), _b.Y()) + epsilon) return Point<double>::Empty();

			return p;
		}
		/*
			std::vector<Point<double>> Intersect(const Rect<double>& r) const;

			rect_side IntersectSide(const Rect<double>& rect) const;
		*/

		Segment<T> operator+(const Point<T>& p) const { return { _a + p, _b + p }; }
		Segment<T> operator-(const Point<T>& p) const { return { _a - p, _b - p }; }

	};

}
