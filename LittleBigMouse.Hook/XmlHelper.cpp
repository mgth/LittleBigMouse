#include "XmlHelper.h"

bool XmlHelper::GetBool(tinyxml2::XMLElement* rectElement, const char* name, bool defaultValue)
{
	auto attribut = rectElement->FindAttribute(name);
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

long XmlHelper::GetLong(tinyxml2::XMLElement* rectElement, const char* name)
{
	auto attribut = rectElement->FindAttribute(name);
	if(attribut)
	{
		return std::stol(attribut->Value());
	}
	return 0;
}

double XmlHelper::GetDouble(tinyxml2::XMLElement* rectElement, const char* name)
{
	auto attribut = rectElement->FindAttribute(name);
	if(attribut)
	{
		return std::stod(attribut->Value());
	}
	return 0.0;
}

std::string XmlHelper::GetString(tinyxml2::XMLElement* rectElement, const char* name)
{
	auto attribut = rectElement->FindAttribute(name);
	if(attribut)
	{
		return std::string(attribut->Value());
	}
	return "";
}

Rect<long> XmlHelper::GetRectLong(tinyxml2::XMLElement* parent, const char* name)
{
	auto element = parent->FirstChildElement(name);
	if(element)
	{
		auto rectElement = element->FirstChildElement("Rect");
		if(rectElement)
		{
			const long left = GetLong(rectElement,"Left");
			const long top = GetLong(rectElement,"Top");
			const long width = GetLong(rectElement,"Width");
			const long height = GetLong(rectElement,"Height");

			return Rect<long>(left,top,width,height);
		}
	}

	return Rect<long>(0,0,0,0);

}

Rect<double> XmlHelper::GetRectDouble(tinyxml2::XMLElement* parent, const char* name)
{
	auto element = parent->FirstChildElement(name);
	if(element)
	{
		auto rectElement = element->FirstChildElement("Rect");
		if(rectElement)
		{
			double left = GetDouble(rectElement,"Left");
			double top = GetDouble(rectElement,"Top");
			double width = GetDouble(rectElement,"Width");
			double height = GetDouble(rectElement,"Height");

			return Rect<double>(left,top,width,height);
		}
	}

	return Rect<double>(0,0,0,0);
}
