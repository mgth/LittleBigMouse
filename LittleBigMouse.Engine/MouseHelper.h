#pragma once
#include "Point.h"
#include "Rect.h"

void SetMouseLocation(const geo::Point<long>& location);
geo::Point<long> GetMouseLocation();
void SetClip(const geo::Rect<long>& r);
geo::Rect<long> GetClip();
