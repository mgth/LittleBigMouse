#include <string>
#include <Windows.h>
#include "str.h"

std::string to_string(const std::wstring& wstr) 
{ 
    int strLength  = WideCharToMultiByte(
        CP_UTF8, 0, wstr.c_str(), -1, 
        nullptr, 0, nullptr, nullptr
    ); 
    std::string str(strLength, 0); 
  
    WideCharToMultiByte(
        CP_UTF8, 0, wstr.c_str(), -1, &str[0], 
        strLength, nullptr, nullptr
    ); 
    return str; 
} 

std::wstring to_wstring(const std::string& str) 
{ 
    int strLength = MultiByteToWideChar(
        CP_UTF8, 0, str.c_str(), -1, 
        nullptr, 0
    ); 
    std::wstring wstr(strLength, 0); 
  
    MultiByteToWideChar(
        CP_UTF8, 0, str.c_str(), -1, &wstr[0], 
        strLength
    ); 
    return wstr; 
} 