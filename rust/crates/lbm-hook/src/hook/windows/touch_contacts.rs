//! Bounded HID contact decoding for native focus restoration. Unknown report
//! formats fail closed; the independent mouse fallback does not depend on this.
use windows::Win32::Devices::HumanInterfaceDevice::{
    HidP_GetButtonCaps, HidP_GetCaps, HidP_GetUsageValue, HidP_GetUsages, HidP_Input,
    HIDP_BUTTON_CAPS, HIDP_CAPS, HIDP_STATUS_SUCCESS, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Input::{GetRawInputDeviceInfoW, RIDI_PREPARSEDDATA};

const MAX_CONTACTS: usize = 128;

/// Hybrid devices split one frame across reports. A zero contact count means
/// continuation, not finger-up. Publish only after the whole frame is decoded.
#[derive(Default)]
struct Frame {
    remaining: usize,
    active: bool,
}

impl Frame {
    fn start(&mut self, count: usize) -> Option<usize> {
        if count > MAX_CONTACTS {
            *self = Self::default();
            return None;
        }
        if count != 0 {
            self.remaining = count;
            self.active = false;
        }
        (self.remaining != 0).then_some(self.remaining)
    }

    fn push(&mut self, tip: bool) -> Option<bool> {
        if self.remaining == 0 {
            return None;
        }
        self.active |= tip;
        self.remaining -= 1;
        (self.remaining == 0).then_some(self.active)
    }
}

pub struct Decoder {
    // Word-aligned storage owns the preparsed descriptor; no OS handle is opened.
    descriptor: Vec<usize>,
    tips: Vec<(u8, u16)>,
    frame: Frame,
}

impl Decoder {
    pub fn new(device: HANDLE) -> Option<Self> {
        let mut size = 0;
        if unsafe { GetRawInputDeviceInfoW(device, RIDI_PREPARSEDDATA, None, &mut size) }
            == u32::MAX
            || size == 0
            || size > 65536
        {
            return None;
        }
        let mut descriptor = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
        if unsafe {
            GetRawInputDeviceInfoW(
                device,
                RIDI_PREPARSEDDATA,
                Some(descriptor.as_mut_ptr().cast()),
                &mut size,
            )
        } == u32::MAX
        {
            return None;
        }
        let data = PHIDP_PREPARSED_DATA(descriptor.as_ptr() as isize);
        let mut caps = HIDP_CAPS::default();
        if unsafe { HidP_GetCaps(data, &mut caps) } != HIDP_STATUS_SUCCESS
            || caps.NumberInputButtonCaps == 0
            || caps.NumberInputButtonCaps > 512
        {
            return None;
        }
        let mut buttons = vec![HIDP_BUTTON_CAPS::default(); caps.NumberInputButtonCaps as usize];
        let mut length = caps.NumberInputButtonCaps;
        if unsafe { HidP_GetButtonCaps(HidP_Input, buttons.as_mut_ptr(), &mut length, data) }
            != HIDP_STATUS_SUCCESS
        {
            return None;
        }
        let mut tips = Vec::new();
        for button in &buttons[..length as usize] {
            let contains_tip = unsafe {
                if button.IsRange.as_bool() {
                    button.Anonymous.Range.UsageMin <= 0x42
                        && button.Anonymous.Range.UsageMax >= 0x42
                } else {
                    button.Anonymous.NotRange.Usage == 0x42
                }
            };
            if button.UsagePage == 0x0d && contains_tip {
                let slot = (button.ReportID, button.LinkCollection);
                if !tips.contains(&slot) {
                    tips.push(slot);
                }
            }
        }
        if tips.is_empty() || tips.len() > MAX_CONTACTS {
            return None;
        }
        Some(Self {
            descriptor,
            tips,
            frame: Frame::default(),
        })
    }

    pub fn contact(&mut self, report: &mut [u8]) -> Option<bool> {
        let id = *report.first()?;
        let data = PHIDP_PREPARSED_DATA(self.descriptor.as_ptr() as isize);
        let mut count = 0;
        if unsafe { HidP_GetUsageValue(HidP_Input, 0x0d, 0, 0x54, &mut count, data, report) }
            != HIDP_STATUS_SUCCESS
        {
            self.frame = Frame::default();
            return None;
        }
        let remaining = self.frame.start(count as usize)?;
        let mut result = None;
        for (_, link) in self.tips.iter().filter(|slot| slot.0 == id).take(remaining) {
            let mut usages = [0u16; 128];
            let mut length = usages.len() as u32;
            if unsafe {
                HidP_GetUsages(
                    HidP_Input,
                    0x0d,
                    *link,
                    usages.as_mut_ptr(),
                    &mut length,
                    data,
                    report,
                )
            } != HIDP_STATUS_SUCCESS
            {
                self.frame = Frame::default();
                return None;
            }
            result = self.frame.push(usages[..length as usize].contains(&0x42));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parallel_contacts_release_only_when_every_tip_is_up() {
        let mut frame = Frame::default();
        frame.start(2);
        assert_eq!(frame.push(false), None);
        assert_eq!(frame.push(true), Some(true));
        frame.start(2);
        assert_eq!(frame.push(false), None);
        assert_eq!(frame.push(false), Some(false));
    }
    #[test]
    fn hybrid_zero_count_continues_the_frame() {
        let mut frame = Frame::default();
        assert_eq!(frame.start(0), None);
        frame.start(2);
        assert_eq!(frame.push(true), None);
        assert_eq!(frame.start(0), Some(1));
        assert_eq!(frame.push(false), Some(true));
        frame.start(1);
        assert_eq!(frame.push(false), Some(false));
        assert_eq!(frame.start(MAX_CONTACTS + 1), None);
    }
}
