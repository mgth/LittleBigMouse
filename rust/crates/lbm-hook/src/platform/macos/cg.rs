//! The handful of CoreGraphics / CoreFoundation / ApplicationServices calls the
//! macOS backend needs, declared by hand.
//!
//! Declared here rather than pulled from the `core-graphics` / `core-foundation`
//! crates on purpose: the surface is small (an event tap, a warp, the display
//! list, a few input-state queries) and stable since 10.4, and keeping it local
//! means the daemon adds no dependency for one target, exactly like the Linux
//! backends talk to libc directly.
//!
//! Type widths follow the SDK headers: `boolean_t` is `int`, CF's `Boolean` is
//! `unsigned char`, and the newer CoreGraphics prototypes take and return C99
//! `bool`, which Rust's `bool` matches.

#![allow(non_upper_case_globals, non_camel_case_types, dead_code)]

use std::ffi::c_void;

pub type CGDirectDisplayID = u32;
pub type CGError = i32;
pub type CGEventType = u32;
pub type CGEventField = u32;
pub type CGEventMask = u64;
pub type CGEventFlags = u64;
pub type CGEventSourceStateID = i32;
pub type CGMouseButton = u32;
pub type CGEventTapLocation = u32;
pub type CGEventTapPlacement = u32;
pub type CGEventTapOptions = u32;

pub type CFTypeRef = *const c_void;
pub type CFAllocatorRef = *const c_void;
pub type CFStringRef = *const c_void;
pub type CFDictionaryRef = *const c_void;
pub type CFMachPortRef = *mut c_void;
pub type CFRunLoopRef = *mut c_void;
pub type CFRunLoopSourceRef = *mut c_void;
pub type CFIndex = isize;
pub type Boolean = u8;

pub type CGEventRef = *mut c_void;
pub type CGEventSourceRef = *mut c_void;
pub type CGEventTapProxy = *mut c_void;
pub type CGDisplayModeRef = *mut c_void;

pub type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

/// Opaque: only ever passed by address (`&kCFTypeDictionaryKeyCallBacks`).
#[repr(C)]
pub struct CFDictionaryCallBacks {
    _opaque: [u8; 0],
}

// --- constants (CGEventTypes.h, CGEventSource.h, CGRemoteOperation.h) ---------

pub const kCGSessionEventTap: CGEventTapLocation = 1;
pub const kCGHeadInsertEventTap: CGEventTapPlacement = 0;
pub const kCGEventTapOptionDefault: CGEventTapOptions = 0;

pub const kCGEventMouseMoved: CGEventType = 5;
pub const kCGEventLeftMouseDragged: CGEventType = 6;
pub const kCGEventRightMouseDragged: CGEventType = 7;
pub const kCGEventOtherMouseDragged: CGEventType = 27;
/// Pseudo-events delivered to the tap itself when the system turned it off.
pub const kCGEventTapDisabledByTimeout: CGEventType = 0xFFFF_FFFE;
pub const kCGEventTapDisabledByUserInput: CGEventType = 0xFFFF_FFFF;

pub const kCGMouseEventDeltaX: CGEventField = 4;
pub const kCGMouseEventDeltaY: CGEventField = 5;

pub const kCGEventSourceStateCombinedSessionState: CGEventSourceStateID = 0;
pub const kCGEventSourceStateHIDSystemState: CGEventSourceStateID = 1;

pub const kCGEventFlagMaskControl: CGEventFlags = 0x0004_0000;

pub const kCFRunLoopRunFinished: i32 = 1;

pub const kCGErrorSuccess: CGError = 0;

pub const fn event_mask_bit(event_type: CGEventType) -> CGEventMask {
    1 << event_type
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: CGEventTapPlacement,
        options: CGEventTapOptions,
        events_of_interest: CGEventMask,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;

    pub fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
    pub fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    pub fn CGEventSetLocation(event: CGEventRef, location: CGPoint);
    pub fn CGEventGetDoubleValueField(event: CGEventRef, field: CGEventField) -> f64;

    pub fn CGEventSourceCreate(state_id: CGEventSourceStateID) -> CGEventSourceRef;
    pub fn CGEventSourceSetLocalEventsSuppressionInterval(source: CGEventSourceRef, seconds: f64);
    pub fn CGEventSourceFlagsState(state_id: CGEventSourceStateID) -> CGEventFlags;
    pub fn CGEventSourceButtonState(state_id: CGEventSourceStateID, button: CGMouseButton) -> bool;

    pub fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> CGError;
    pub fn CGAssociateMouseAndMouseCursorPosition(connected: u32) -> CGError;

    pub fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> CGError;
    pub fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;

    /// 10.15+: whether the process may tap / post events (Accessibility).
    pub fn CGPreflightPostEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopDefaultMode: CFStringRef;
    pub static kCFBooleanTrue: CFTypeRef;
    pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryCallBacks;
    pub static kCFTypeDictionaryValueCallBacks: CFDictionaryCallBacks;

    pub fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    pub fn CFMachPortInvalidate(port: CFMachPortRef);
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopRemoveSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: f64,
        return_after_source_handled: Boolean,
    ) -> i32;
    pub fn CFDictionaryCreate(
        allocator: CFAllocatorRef,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: CFIndex,
        key_callbacks: *const CFDictionaryCallBacks,
        value_callbacks: *const CFDictionaryCallBacks,
    ) -> CFDictionaryRef;
    pub fn CFRelease(cf: CFTypeRef);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub static kAXTrustedCheckOptionPrompt: CFStringRef;
    pub fn AXIsProcessTrusted() -> Boolean;
    pub fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> Boolean;
}

/// The current cursor position, in global display points (origin at the top-left
/// of the main display, y down — the same orientation as Win32 screen pixels).
pub fn cursor_location() -> CGPoint {
    unsafe {
        let event = CGEventCreate(std::ptr::null_mut());
        if event.is_null() {
            return CGPoint::default();
        }
        let p = CGEventGetLocation(event);
        CFRelease(event as CFTypeRef);
        p
    }
}

/// Bounds of every active display, in global points.
pub fn active_display_bounds() -> Vec<CGRect> {
    const MAX: usize = 32;
    let mut ids = [0 as CGDirectDisplayID; MAX];
    let mut count = 0u32;
    let err = unsafe { CGGetActiveDisplayList(MAX as u32, ids.as_mut_ptr(), &mut count) };
    if err != kCGErrorSuccess {
        return Vec::new();
    }
    ids[..count as usize]
        .iter()
        .map(|id| unsafe { CGDisplayBounds(*id) })
        .collect()
}

/// Ask for Accessibility trust, showing the system prompt once if it is missing.
/// Returns whether the process is trusted right now (a fresh grant only takes
/// effect for event taps created after it, so the caller retries later).
pub fn ensure_accessibility_prompt() -> bool {
    unsafe {
        if AXIsProcessTrusted() != 0 {
            return true;
        }
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        );
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        if !options.is_null() {
            CFRelease(options);
        }
        trusted
    }
}
