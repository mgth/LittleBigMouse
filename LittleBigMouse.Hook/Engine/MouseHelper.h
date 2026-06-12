#pragma once
#include "Framework.h"

#include "Geometry/Point.h"
#include "Geometry/Rect.h"

void SetMouseLocation(const geo::Point<long>& location);
geo::Point<long> GetMouseLocation();
void SetClip(const geo::Rect<long>& r);
void ClearClip();
geo::Rect<long> GetClip();
