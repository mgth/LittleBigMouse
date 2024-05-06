#pragma once

#include <vector>

#include "Line.h"
#include "Point.h"
#include "Segment.h"

namespace geo
{
	template<class T>
	class Rect
	{
		T _left;
		T _top;
		T _width;
		T _height;
	public:
		[[nodiscard]] T Left() const { return _left; }
		[[nodiscard]] T Top() const { return _top; }
		[[nodiscard]] T Width() const { return _width; }
		[[nodiscard]] T Height() const { return _height; }
		[[nodiscard]] T Right() const { return _left + _width; }
		[[nodiscard]] T Bottom() const { return _top + _height; }

		[[nodiscard]] bool Contains(const Point<T>& point) const;

		[[nodiscard]] std::vector<Point<T>> Intersect(const Line<T>& l) const
		{
			std::vector<Point<T>> result;

			Point<T> p;
			if (Segment<T>(TopLeft(), BottomLeft()).IsIntersecting(l, p))
			{
				result.push_back(p);
			}
			if (Segment<T>(TopLeft(), TopRight()).IsIntersecting(l, p))
			{
				result.push_back(p);
			}
			if (Segment<T>(TopRight(), BottomRight()).IsIntersecting(l, p))
			{
				result.push_back(p);
			}
			if (Segment<T>(BottomLeft(), BottomRight()).IsIntersecting(l, p))
			{
				result.push_back(p);
			}
			return result;
		}

		bool operator==(const Rect& target) const
		{
			return
				_left == target._left
				&& _top == target._top
				&& _width == target._width
				&& _height == target._height;
		}

		bool operator!=(const Rect& target) const
		{
			return !operator==(target);
		}

		[[nodiscard]] Point<T> TopLeft() const
		{
			return { Left(),Top() };
		}

		[[nodiscard]] Point<T> TopRight() const
		{
			return { Right(),Top() };
		}

		[[nodiscard]] Point<T> BottomLeft() const
		{
			return { Left(),Bottom() };
		}

		[[nodiscard]] Point<T> BottomRight() const
		{
			return { Right(),Bottom() };
		}

		static Rect<T> Empty() { return Rect(0, 0, 0, 0); }

		bool IsEmpty() {return _width == 0 || _height == 0;}

#define max(a, b)  (((a) > (b)) ? (a) : (b))
#define min(a, b)  (((a) < (b)) ? (a) : (b))

		Rect(const Point<T> p1, const Point<T> p2) : _left(min(p1.X(), p2.X())), _top(min(p1.Y(), p2.Y())), _width(max(p1.X(), p2.X()) - _left), _height(max(p1.Y(), p2.Y()) - _top)
		{
		}

		Rect(const T left, const T top, const T width, const T height) : _left(left), _top(top), _width(width), _height(height)
		{
		}

		friend std::ostream& operator<<(std::ostream& os, const Rect<T>& r) { return os << "[" << r.TopLeft() << r.Width << "," <<  r.Height << "]";}

	};

	template<class T>
	bool Rect<T>::Contains(const Point<T>& point) const
	{
		return (
			(point.X() >= _left)
			&&
			(point.X() - _width < _left)
			&&
			(point.Y() >= _top)
			&&
			(point.Y() - _height < _top)
			);

	}


}




