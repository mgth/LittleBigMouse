#pragma once
#include <string>

#include "Rect.h"
#include "tinyxml2.h"


class XmlHelper
{
public:
	static bool GetBool(tinyxml2::XMLElement* rectElement, const char* name, bool defaultValue = false);

	static long GetLong(tinyxml2::XMLElement* rectElement, const char* name);

	static double GetDouble(tinyxml2::XMLElement* rectElement, const char* name);

	static std::string GetString(tinyxml2::XMLElement* rectElement, const char* name);

	static Rect<long> GetRectLong(tinyxml2::XMLElement* parent, const char* name);

	static Rect<double> GetRectDouble(tinyxml2::XMLElement* parent, const char* name);
};

