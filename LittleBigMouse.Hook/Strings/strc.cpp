#include <string>
#include <Windows.h>
#include "str.h"


std::wstring ToWString(const std::string& str)
{
	std::wstring wstr;
	size_t size;
	wstr.resize(str.length());
	(void)mbstowcs_s(&size, wstr.data(),wstr.size()+1,str.c_str(),str.size());
	return wstr;
}

std::string ToString(const std::wstring& wstr)
{
	std::string str;
	size_t size;
	str.resize(wstr.length());
	(void)wcstombs_s(&size, str.data(), str.size() + 1, wstr.c_str(), wstr.size());
	return str;
}


std::string ToString2(const std::wstring& ws) 
{
    const size_t wLength = ws.size();
    if (wLength > INT_MAX ) return {};
    const int wLengthInt = static_cast<int>(wLength);

	const int length  = WideCharToMultiByte(
        CP_UTF8, 0, ws.data(), wLengthInt, 
        nullptr, 0, nullptr, nullptr
    ); 
    std::string s(length, 0); 
  
    WideCharToMultiByte(
        CP_UTF8, 0, ws.data(), wLengthInt, s.data(), 
        length, nullptr, nullptr
    ); 
    return s; 
} 

std::wstring ToWString2(const std::string& s) 
{ 
    const size_t sLength = s.size();
    if (sLength > INT_MAX ) return {};
    const int sLengthInt = static_cast<int>(sLength);

    const int length = MultiByteToWideChar(
        CP_UTF8, 0, s.data(), sLengthInt, 
        nullptr, 0
    ); 
    std::wstring ws(length, 0); 
  
    MultiByteToWideChar(
        CP_UTF8, 0, s.data(), sLengthInt, ws.data(), 
        length
    ); 
    return ws; 
} 