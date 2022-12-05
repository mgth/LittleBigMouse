#pragma once
#include <vector>
#include <algorithm>
#include <iterator>

#include "Point.h"

template<class T>
class Rect
{
	T _left;
	T _top;
	T _width;
	T _height;
public:
	T Left() const {return _left;}
	T Top() const {return _top;}
	T Width() const {return _width;}
	T Height() const {return _height;}
	T Right() const {return _left + _width;}
	T Bottom() const {return _top + _height;}

	bool Contains(const Point<T>& point) const;

	std::vector<Rect<T>> Travel(Rect target, const std::vector<Rect<long>>& allowed) const;
	std::vector<Rect<T>> Reachable(Rect target) const;

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

	Rect(const double left, const double top, const double width, const double height): _left(left), _top(top), _width(width), _height(height)
	{
	}
};

template<class T>
bool Rect<T>::Contains(const Point<T>& point) const
{
            return (
				(point.X() >= _left) 
				&& 
				(point.X() - _width <= _left) 
				&&
                (point.Y() >= _top) 
				&& 
				(point.Y() - _height <= _top)
			);

}



