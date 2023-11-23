#pragma once
#include <string>

#include "Rect.h"
#include "tinyxml2.h"


class XmlHelper
{
public:
	static bool GetBool(tinyxml2::XMLElement* rectElement, const char* name, bool defaultValue = false);

	static long GetLong(const tinyxml2::XMLElement* rectElement, const char* name);
	static int GetInt(const tinyxml2::XMLElement* element, const char* name);

	static double GetDouble(const tinyxml2::XMLElement* element, const char* name);

	static std::string GetString(const tinyxml2::XMLElement* element, const char* name);

	static geo::Rect<long> GetRectLong(tinyxml2::XMLElement* parent, const char* name);

	static geo::Rect<double> GetRectDouble(tinyxml2::XMLElement* parent, const char* name);
};

