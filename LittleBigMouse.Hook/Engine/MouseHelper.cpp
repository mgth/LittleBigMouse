#include "MouseHelper.h"

geo::Point<long> GetMouseLocation()
{
	POINT p;
	if(GetCursorPos(&p)) return {p.x,p.y};
	return geo::Point<long>::Empty();
}

void SetClip(const geo::Rect<long>& r)
{
#if _DEBUG
	if (r.IsEmpty()) {LOG_DEBUG("r is EMPTY");}
	else if (r.Width() < 0 || r.Height() < 0) {LOG_DEBUG("r is negative");}
	else if (r.Width() < 100 || r.Height() < 100) {LOG_DEBUG("r is small");}
#endif
	const auto rect = RECT{r.Left(),r.Top(),r.Right(),r.Bottom()};
	ClipCursor(&rect);
}

geo::Rect<long> GetClip()
{
	RECT r;
	if(GetClipCursor(&r))
	{
		return geo::Rect(r.left,r.top,r.right-r.left,r.bottom-r.top);
	}
	return geo::Rect<long>::Empty();
}

static void send_mouse_input(DWORD dwFlags, DWORD dx, DWORD dy, DWORD dwData)
{
	INPUT inp;
	inp.type = INPUT_MOUSE;
	inp.mi.dwFlags = dwFlags;
	inp.mi.dx = dx;
	inp.mi.dy = dy;
	inp.mi.mouseData = dwData;
	inp.mi.time = 0;
	inp.mi.dwExtraInfo = 0;
	SendInput(1, &inp, sizeof(inp));
}

static void deskMouseMove(int x, int y)
{
    // when using absolute positioning with mouse_event(),
    // the normalized device coordinates range over only
    // the primary screen.
    const int w = GetSystemMetrics(SM_CXSCREEN);
    const int h = GetSystemMetrics(SM_CYSCREEN);
    send_mouse_input(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                            (DWORD)((65535.0f * x) / (w - 1) + 0.5f),
                            (DWORD)((65535.0f * y) / (h - 1) + 0.5f),
                            0);
}


void SetMouseLocation(const geo::Point<long>& location)
{
	SetCursorPos(location.X(),location.Y());

	//deskMouseMove(location.X(),location.Y());
}



