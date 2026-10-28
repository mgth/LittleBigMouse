//! Observe only the touchscreen HID collection, including apps consuming native
//! touch without mouse promotion. INPUTSINK neither redirects nor suppresses input.
//! Only bounded touchscreen descriptors are retained. No keyboard input is registered.
use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RegisterRawInputDevices, HRAWINPUT, RAWINPUTDEVICE,
    RAWINPUTHEADER, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE, RIDI_DEVICEINFO,
    RID_DEVICE_INFO, RID_HEADER, RID_INPUT, RIM_TYPEHID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorInfo, KillTimer, SetTimer, CURSORINFO, CURSOR_SUPPRESSED,
};

pub const RELEASE_TIMER: usize = 0x4c424d54;
const MAX_PACKET: usize = 65536;
const MAX_DEVICES: usize = 8;
const MAX_REPORTS: usize = 32;
const CURSOR_SETTLE_MS: u32 = 50;

struct Device {
    key: isize,
    decoder: Option<super::touch_contacts::Decoder>,
    in_contact: bool,
}

#[derive(Default)]
struct Reports {
    packet: Vec<usize>,
    devices: Vec<Device>,
}
thread_local! {
    static REPORTS: RefCell<Reports> = RefCell::new(Reports::default());
    // A positive-only cache avoids repeating the device-info query for every
    // report from the same touchscreen. Hotplug notifications invalidate it.
    static LAST_TOUCH_DEVICE: Cell<Option<isize>> = const { Cell::new(None) };
}

pub fn devices_changed() {
    LAST_TOUCH_DEVICE.with(|v| v.set(None));
    REPORTS.with(|v| *v.borrow_mut() = Reports::default());
    super::focus_restore::cancel_native();
}

pub fn on_timer(hwnd: HWND) {
    let _ = unsafe { KillTimer(hwnd, RELEASE_TIMER) };
    super::touch_input::finish_native_focus();
}

pub fn register(hwnd: HWND) {
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x0d,
        usUsage: 0x04,
        dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
        hwndTarget: hwnd,
    };
    if let Err(error) =
        unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
    {
        eprintln!("[LittleBigMouse.Hook] native touch registration failed: {error}");
    }
}

pub fn unregister() {
    devices_changed();
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x0d,
        usUsage: 0x04,
        dwFlags: RIDEV_REMOVE,
        hwndTarget: HWND::default(),
    };
    let _ = unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) };
}

pub fn on_input(hwnd: HWND, lparam: LPARAM) {
    let mut header = RAWINPUTHEADER::default();
    let mut size = size_of::<RAWINPUTHEADER>() as u32;
    let read = unsafe {
        GetRawInputData(
            HRAWINPUT(lparam.0 as *mut _),
            RID_HEADER,
            Some((&mut header as *mut RAWINPUTHEADER).cast()),
            &mut size,
            size_of::<RAWINPUTHEADER>() as u32,
        )
    };
    if read != size_of::<RAWINPUTHEADER>() as u32 || header.dwType != RIM_TYPEHID.0 {
        return;
    }
    if !is_touchscreen(header.hDevice) {
        return;
    }
    // Native touch moves the cursor without WH_MOUSE_LL activity; count it so
    // the watchdog does not reinstall the hook and discard the saved anchor.
    crate::hook::hot_path::count_event();
    let Some(_shared) = crate::shared::SHARED.get().filter(|s| s.touch_active()) else {
        return;
    };
    let mut cursor = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetCursorInfo(&mut cursor) }.is_ok() && cursor.flags.0 & CURSOR_SUPPRESSED.0 != 0 {
        // Ignore idle HID reports while the mouse owns the visible cursor.
        super::touch_input::on_native_touch();
    }
    // Contact boundaries also re-arm cursor restoration after an override,
    // even when keyboard-focus restoration is disabled.
    // Decode without holding any RefCell borrow while invoking focus/Win32 UI
    // operations, which may re-enter the low-level mouse hook.
    let contacts = REPORTS.with(|storage| storage.borrow_mut().read(lparam, &header));
    for contact in contacts.into_iter().flatten() {
        if contact {
            let _ = unsafe { KillTimer(hwnd, RELEASE_TIMER) };
        }
        if super::touch_input::on_native_contact(contact) {
            // Cursor relocation can arrive after the HID release.
            unsafe {
                SetTimer(hwnd, RELEASE_TIMER, CURSOR_SETTLE_MS, None);
            }
        }
    }
}

fn is_touchscreen(device: windows::Win32::Foundation::HANDLE) -> bool {
    let key = device.0 as isize;
    if LAST_TOUCH_DEVICE.with(|v| v.get() == Some(key)) {
        return true;
    }
    let mut info = RID_DEVICE_INFO {
        cbSize: size_of::<RID_DEVICE_INFO>() as u32,
        ..Default::default()
    };
    let mut size = info.cbSize;
    if unsafe {
        GetRawInputDeviceInfoW(
            device,
            RIDI_DEVICEINFO,
            Some((&mut info as *mut RID_DEVICE_INFO).cast()),
            &mut size,
        )
    } == u32::MAX
        || info.dwType != RIM_TYPEHID
    {
        return false;
    }
    let hid = unsafe { info.Anonymous.hid };
    if hid.usUsagePage != 0x0d || hid.usUsage != 0x04 {
        return false;
    }
    LAST_TOUCH_DEVICE.with(|v| v.set(Some(key)));
    true
}

impl Reports {
    fn read(&mut self, lparam: LPARAM, header: &RAWINPUTHEADER) -> [Option<bool>; MAX_REPORTS] {
        let mut result = [None; MAX_REPORTS];
        let size = header.dwSize as usize;
        let header_size = size_of::<RAWINPUTHEADER>();
        if !(header_size + 8..=MAX_PACKET).contains(&size) {
            return result;
        }
        let key = header.hDevice.0 as isize;
        let index = match self.devices.iter().position(|device| device.key == key) {
            Some(index) => index,
            None if self.devices.len() < MAX_DEVICES => {
                self.devices.push(Device {
                    key,
                    decoder: super::touch_contacts::Decoder::new(header.hDevice),
                    in_contact: false,
                });
                self.devices.len() - 1
            }
            None => return result,
        };
        let Some(decoder) = self.devices[index].decoder.as_mut() else {
            return result;
        };
        self.packet.resize(size.div_ceil(size_of::<usize>()), 0);
        let mut read_size = size as u32;
        if unsafe {
            GetRawInputData(
                HRAWINPUT(lparam.0 as *mut _),
                RID_INPUT,
                Some(self.packet.as_mut_ptr().cast()),
                &mut read_size,
                header_size as u32,
            )
        } != size as u32
        {
            return result;
        }
        let bytes =
            unsafe { std::slice::from_raw_parts_mut(self.packet.as_mut_ptr().cast::<u8>(), size) };
        let Some(reports) = split_reports(&mut bytes[header_size..]) else {
            return result;
        };
        for (index, report) in reports.enumerate() {
            result[index] = decoder.contact(report);
        }
        for contact in result.iter_mut().flatten() {
            self.devices[index].in_contact = *contact;
            *contact = self.devices.iter().any(|device| device.in_contact);
        }
        result
    }
}

fn split_reports(body: &mut [u8]) -> Option<std::slice::ChunksExactMut<'_, u8>> {
    let report_size = u32::from_le_bytes(body.get(..4)?.try_into().ok()?) as usize;
    let count = u32::from_le_bytes(body.get(4..8)?.try_into().ok()?) as usize;
    let data = body.get_mut(8..)?;
    if report_size == 0 || count > MAX_REPORTS || report_size.checked_mul(count) != Some(data.len())
    {
        return None;
    }
    Some(data.chunks_exact_mut(report_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packet_boundaries_are_validated_before_hid_decoding() {
        let mut packet = vec![2, 0, 0, 0, 2, 0, 0, 0, 10, 11, 12, 13];
        let reports: Vec<_> = split_reports(&mut packet)
            .unwrap()
            .map(|r| r.to_vec())
            .collect();
        assert_eq!(reports, [vec![10, 11], vec![12, 13]]);
        for length in 0..packet.len() {
            assert!(split_reports(&mut packet[..length]).is_none());
        }
        packet[0] = 0;
        assert!(split_reports(&mut packet).is_none());
        packet[0] = 2;
        packet[4] = (MAX_REPORTS + 1) as u8;
        assert!(split_reports(&mut packet).is_none());
    }
    #[test]
    fn device_changes_discard_cached_identity_and_buffers() {
        LAST_TOUCH_DEVICE.with(|v| v.set(Some(123)));
        REPORTS.with(|v| v.borrow_mut().packet.resize(8192, 0));
        devices_changed();
        assert_eq!(LAST_TOUCH_DEVICE.with(Cell::get), None);
        REPORTS.with(|v| assert_eq!(v.borrow().packet.capacity(), 0));
    }
}
