//! Real Win32 implementation of [`CursorEnv`] — port of `Engine/MouseHelper` plus
//! the freelook signals from `MouseEngine::IsFreelookActive`.

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL};
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, GetClipCursor, GetCursorInfo, GetCursorPos, GetSystemMetrics, SetCursorPos,
    CURSORINFO, CURSOR_SHOWING, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN,
};
use windows::Win32::System::SystemInformation::GetTickCount64;

use crate::engine::cursor::CursorEnv;
use crate::geometry::{Point, Rect};

pub struct Win32Cursor;

impl CursorEnv for Win32Cursor {
    fn get_mouse_location(&self) -> Point<i32> {
        let mut p = POINT::default();
        if unsafe { GetCursorPos(&mut p) }.is_ok() {
            return Point::new(p.x, p.y);
        }
        Point::empty()
    }

    fn set_mouse_location(&mut self, location: Point<i32>) {
        unsafe {
            let _ = SetCursorPos(location.x(), location.y());
        }
    }

    fn get_clip(&self) -> Rect<i32> {
        let mut r = RECT::default();
        if unsafe { GetClipCursor(&mut r) }.is_ok() {
            return Rect::new(r.left, r.top, r.right - r.left, r.bottom - r.top);
        }
        Rect::empty()
    }

    fn set_clip(&mut self, r: Rect<i32>) {
        let rect = RECT {
            left: r.left(),
            top: r.top(),
            right: r.right(),
            bottom: r.bottom(),
        };
        unsafe {
            let _ = ClipCursor(Some(&rect));
        }
    }

    fn ctrl_down(&self) -> bool {
        (unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) } as u16 & 0x8000) == 0x8000
    }

    fn cursor_hidden(&self) -> bool {
        let mut ci = CURSORINFO {
            cbSize: std::mem::size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        unsafe {
            let _ = GetCursorInfo(&mut ci);
        }
        (ci.flags.0 & CURSOR_SHOWING.0) == 0
    }

    fn clip_is_subrect_of_virtual_screen(&self) -> bool {
        let mut clip = RECT::default();
        unsafe {
            let _ = GetClipCursor(&mut clip);
            let vs_left = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vs_top = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let vs_right = vs_left + GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vs_bottom = vs_top + GetSystemMetrics(SM_CYVIRTUALSCREEN);
            clip.left > vs_left || clip.top > vs_top || clip.right < vs_right || clip.bottom < vs_bottom
        }
    }

    fn tick_count(&self) -> u64 {
        unsafe { GetTickCount64() }
    }
}
