#pragma once
#include "Windows.h"

template<class T>
class Point
{
	double _x;
	double _y;
public:
	T X() const {return _x;}
	T Y() const {return _y;}

	Point(const T x, const T y):_x(x),_y(y)
	{}

	Point(const POINT& p):_x(p.x),_y(p.y){}
};

