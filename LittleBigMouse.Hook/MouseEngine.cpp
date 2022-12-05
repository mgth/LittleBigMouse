#include "MouseEngine.h"
#include "MouseHookerWindowsHook.h"
#include <iostream>
#include "tinyxml2.h"
#define DEBUG 1

void MouseEngine::OnMouseMoveExtFirst(HookMouseEventArg& e)
{
	_oldPoint = e.Point;

	_oldZone = Layout.Containing(_oldPoint);

	if(!_oldZone) return;

    OnMouseMoveFunc = &MouseEngine::OnMouseMoveStraight;
}

bool Contains(const RECT& rect, const POINT& pixel)
{
    if (pixel.x < rect.left) return false;
    if (pixel.y < rect.top) return false;
    if (pixel.x >= rect.right) return false;
    if (pixel.y >= rect.bottom) return false;
    return true;
}

const RECT& ToRECT2(const Rect<long>& r)
{
    return RECT{r.Left(),r.Top(),r.Right(),r.Bottom()};
}


void MouseEngine::OnMouseMoveStraight(HookMouseEventArg& e)
{
	const auto pIn = e.Point;
	if(_reset)
	{
		ClipCursor(&_oldClipRect);
		_reset = false;
	}

	if (Contains(ToRECT2(_oldZone->PixelsBounds()), pIn))
    {
        _oldPoint = pIn;
        e.Handled = false;
        return;
    }
	const Point<double> pInMm = _oldZone->ToPhysical(pIn);

        //Debug.WriteLine($"=====");
        //Debug.WriteLine($"Leaving zone : {_oldZone?.Name??"none"} at {pIn.X:0},{pIn.Y:0} ({pInMm.X:0},{pInMm.Y:0}) mm");
#ifdef DEBUG
            std::cout << "Leaving zone : ";
            if(_oldZone) std::cout << _oldZone->Name;
            std::cout << " at " << pIn.x << "," << pIn.y << "(" << pInMm.X() << "," << pInMm.Y() <<")\n";
#endif

	const Zone* zoneOut = nullptr;

        double minDx = 0.0;
        double minDy = 0.0;

        if (pIn.y >= _oldZone->PixelsBounds().Bottom())
        {
			//Debug.WriteLine("by bottom");
			for(const auto zone: Layout.Zones)
			{
                if (zone->PhysicalBounds().Left() > pInMm.X() || zone->PhysicalBounds().Right() < pInMm.X()) continue;

                // Distance to screen
                const double dy = zone->PhysicalBounds().Top() - _oldZone->PhysicalBounds().Bottom();

                if (dy < 0.0) continue; // wrong direction

                if (zoneOut) // one solution already found
                {
                    if (dy > minDy) continue; // zone farer than already found
                }

                minDy = dy;
                zoneOut = zone;
            }
        }
        else if (pIn.y < _oldZone->PixelsBounds().Top())
        {
            //Debug.WriteLine("by top");
			for(const auto zone: Layout.Zones)
            {
                if (zone->PhysicalBounds().Left() > pInMm.X() || zone->PhysicalBounds().Right() < pInMm.X()) continue;

                // Distance to screen
                double dy = zone->PhysicalBounds().Bottom() - _oldZone->PhysicalBounds().Top();

                if (dy > 0) continue; // wrong direction
                if (zoneOut) // one solution already found
                {
                    if (dy < minDy) continue; // zone farer than already found
                }

                minDy = dy;
                zoneOut = zone;
            }
        }

        if (pIn.x >= _oldZone->PixelsBounds().Right())
        {
            //Debug.WriteLine("by right");
        	for(const auto& zone: Layout.Zones)
            {
                if (zone->PhysicalBounds().Top() > pInMm.Y() || zone->PhysicalBounds().Bottom() < pInMm.Y()) continue;

                // Distance to screen
                double dx = zone->PhysicalBounds().Left() - _oldZone->PhysicalBounds().Right();

                if (dx < 0) continue; // wrong direction
                if (zoneOut) // one solution already found
                {
                    if (dx > minDx) continue; // zone farer than already found
                }

                minDx = dx;
                zoneOut = zone;
            }
        }
        else if (pIn.x < _oldZone->PixelsBounds().Left())
        {
            //Debug.WriteLine("by left");
        	for(const auto zone: Layout.Zones)
            {
                if (zone->PhysicalBounds().Top() > pInMm.Y() || zone->PhysicalBounds().Bottom() < pInMm.Y()) continue;
                double dx = zone->PhysicalBounds().Right() - _oldZone->PhysicalBounds().Left();

                if (dx > 0) continue; // wrong direction
                if (zoneOut) // one solution already found
                {
                    if (dx < minDx) continue; // zone farer than already found
                }

                minDx = dx;
                zoneOut = zone;
            }
        }

        if (!zoneOut)
        {
            //Debug.WriteLine($"No zone found : {pIn}");

            auto r = RECT{
            	(_oldZone->PixelsBounds().Left()),
            	(_oldZone->PixelsBounds().Top()),
            	(_oldZone->PixelsBounds().Right()),
                (_oldZone->PixelsBounds().Bottom())
            	};

            GetClipCursor(&_oldClipRect);
            _reset = true;

            ClipCursor(&r);

            e.Handled = false; // when set to true, cursor stick to frame
            return;
        }
        else
        {
            //Debug.WriteLine($"=====");

            auto pMm = Point<double>(pInMm.X() + minDx, pInMm.Y() + minDy);
            auto pOut = zoneOut->ToPixels(pMm);
            pOut = zoneOut->InsidePixelsBounds(pOut);

#ifdef DEBUG
            std::cout << "to : " << zoneOut->Name << " at " << pOut.x << "," << pOut.y << "(" << pMm.X() << "," << pMm.Y() <<")\n";
#endif
            auto travel = _oldZone->TravelPixels(Layout.MainZones,zoneOut);

            //MouseHookerWindowsHook::UnHook();

            _oldZone = zoneOut->Main;
            _oldPoint = pOut;

            auto r = RECT{
                zoneOut->PixelsBounds().Left(),//+1, 
                zoneOut->PixelsBounds().Top(),//+1, 
                zoneOut->PixelsBounds().Right()+1,// + 1,//-1+1
                zoneOut->PixelsBounds().Bottom()+1// + 1,//*-1+1*
            };

            GetClipCursor(&_oldClipRect);
            POINT pos = pIn;

        	for(const auto& rect: travel)
            {
                //Debug.WriteLine($"travel : {z}");
                if(Contains(rect,pos)) continue;

                ClipCursor(&rect);

                GetCursorPos(&pos);

                //LbmMouse.CursorPos = pos = new Point((z.Right + z.Left)/2, (z.Top + z.Bottom)/2);
                if(Contains(rect,pOut)) break;
            }
            
            //LbmMouse.CursorPos = zoneOut.CenterPixel;
            ClipCursor(&r);
            //_reset = true;
            SetCursorPos(pOut.x,pOut.y);
            ClipCursor(&_oldClipRect);
            SetCursorPos(pOut.x,pOut.y);

            //POINT p;
        	//GetCursorPos(&p);

            e.Handled = true;
            //ZoneChanged?.Invoke(this, new ZoneChangeEventArgs(_oldZone, zoneOut));
            return;

        }
}

void MouseEngine::StartThread()
{
    MouseHookerWindowsHook::Start(*this);
}


void MouseEngine::OnMouseMove(HookMouseEventArg& e)
{
    _lock.lock();
    (*this.*OnMouseMoveFunc)(e);
    _lock.unlock();
}

void MouseEngine::Start()
{
    _thread = new std::thread(&MouseEngine::StartThread,this);
}

void MouseEngine::Stop()
{
    if(_thread && _thread->joinable())
    {
	    MouseHookerWindowsHook::Stop = true;
	    _thread->join();
	    OnMouseMoveFunc = &MouseEngine::OnMouseMoveExtFirst;
    }
}
