#include "pch.h"
#include "XmlHelper.h"

bool XmlHelper::GetBool(tinyxml2::XMLElement* rectElement, const char* name, bool defaultValue)
{
	const auto attribut = rectElement->FindAttribute(name);
	if(attribut)
	{
		if(defaultValue)
		{
			if(strcmp(attribut->Value(), "False") ==0)
			{
				return false;
			}
			return true;
		}
		else
		{
			if(strcmp(attribut->Value(), "True") ==0)
			{
				return true;
			}
			return false;
		}
	}

	return defaultValue;
}

long XmlHelper::GetLong(const tinyxml2::XMLElement* rectElement, const char* name)
{
	if(const auto attribut = rectElement->FindAttribute(name))
	{
		return std::stol(attribut->Value());
	}
	return 0;
}

int XmlHelper::GetInt(const tinyxml2::XMLElement* element, const char* name)
{
	if(const auto attribut = element->FindAttribute(name))
	{
		return std::stoi(attribut->Value());
	}
	return 0;
}

double XmlHelper::GetDouble(const tinyxml2::XMLElement* element, const char* name)
{
	const auto attribut = element->FindAttribute(name);
	if(attribut)
	{
		return std::stod(attribut->Value());
	}
	return 0.0;
}

std::string XmlHelper::GetString(const tinyxml2::XMLElement* element, const char* name)
{
	const auto attribute = element->FindAttribute(name);
	if(attribute)
	{
		return {attribute->Value()};
	}
	return "";
}

geo::Rect<long> XmlHelper::GetRectLong(tinyxml2::XMLElement* parent, const char* name)
{
	const auto element = parent->FirstChildElement(name);
	if(element)
	{
		const auto rectElement = element->FirstChildElement("Rect");
		if(rectElement)
		{
			const long left = GetLong(rectElement,"Left");
			const long top = GetLong(rectElement,"Top");
			const long width = GetLong(rectElement,"Width");
			const long height = GetLong(rectElement,"Height");

			return {left,top,width,height};
		}
	}

	return {0,0,0,0};

}

geo::Rect<double> XmlHelper::GetRectDouble(tinyxml2::XMLElement* parent, const char* name)
{
	const auto element = parent->FirstChildElement(name);
	if(element)
	{
		const auto rectElement = element->FirstChildElement("Rect");
		if(rectElement)
		{
			const double left = GetDouble(rectElement,"Left");
			const double top = GetDouble(rectElement,"Top");
			const double width = GetDouble(rectElement,"Width");
			const double height = GetDouble(rectElement,"Height");

			return geo::Rect<double>(left,top,width,height);
		}
	}

	return geo::Rect<double>(0,0,0,0);
}
