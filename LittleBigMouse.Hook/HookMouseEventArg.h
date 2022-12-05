#pragma once
#include <Windows.h>

class HookMouseEventArg
{
public:
	POINT Point;
	bool Handled;
};

