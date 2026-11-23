//! Which DDC/CI handle Windows gives for a monitor of the layout: the pure part of the
//! way from the monitor's device id to a physical monitor of dxva2.
//!
//! The layout knows a Windows monitor by the device id `EnumDisplayDevices` reports
//! for it (`MONITOR\DEL4065\{4d36e96e-…}\0003`), below an adapter source
//! (`\\.\DISPLAY1`). dxva2 hands out physical monitors per `HMONITOR`, and an
//! `HMONITOR` names the adapter source it shows (`MONITORINFOEX.szDevice`); its
//! physical monitors are that source's active monitors, in the order the source lists
//! them — several only when the source is duplicated on several panels.
#![cfg_attr(not(windows), allow(dead_code))]

/// An adapter source and the monitor devices below it, as `EnumDisplayDevices` lists
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AdapterMonitors {
    /// `\\.\DISPLAY1`.
    pub name: String,
    /// Each monitor's device id, and whether it is active (`DISPLAY_DEVICE_ACTIVE`).
    pub monitors: Vec<(String, bool)>,
}

/// Where a monitor's DDC/CI handle is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Located {
    /// The rank of its `HMONITOR` in what `EnumDisplayMonitors` gave.
    pub hmonitor: usize,
    /// Its rank among that `HMONITOR`'s physical monitors.
    pub physical: usize,
    /// How many physical monitors that `HMONITOR` must have for the rank to mean this
    /// monitor: any other count is a source arranged in a way this cannot read.
    pub expected: usize,
}

/// Finds the handle of the monitor `device_id` among `adapters`, whose `HMONITOR`s'
/// source names are `hmonitors`. Device ids are compared without regard to case, as
/// Windows treats them.
pub(super) fn locate(
    adapters: &[AdapterMonitors],
    hmonitors: &[String],
    device_id: &str,
) -> Result<Located, String> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let adapter = adapters
        .iter()
        .find(|a| a.monitors.iter().any(|(id, _)| same(id, device_id)))
        .ok_or("This monitor is no longer connected.")?;
    let active: Vec<&String> = adapter
        .monitors
        .iter()
        .filter(|(_, active)| *active)
        .map(|(id, _)| id)
        .collect();
    let physical = active
        .iter()
        .position(|id| same(id, device_id))
        .ok_or("This monitor is not on the desktop: Windows offers no DDC/CI handle for it.")?;
    let hmonitor = hmonitors
        .iter()
        .position(|name| same(name, &adapter.name))
        .ok_or("This monitor is not on the desktop: Windows offers no DDC/CI handle for it.")?;
    Ok(Located {
        hmonitor,
        physical,
        expected: active.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELL: &str = r"MONITOR\DEL4065\{4d36e96e-e325-11ce-bfc1-08002be10318}\0003";
    const LG: &str = r"MONITOR\GSM5B7F\{4d36e96e-e325-11ce-bfc1-08002be10318}\0004";
    const OLD: &str = r"MONITOR\SAM0F00\{4d36e96e-e325-11ce-bfc1-08002be10318}\0001";

    fn adapters() -> Vec<AdapterMonitors> {
        vec![
            AdapterMonitors {
                name: r"\\.\DISPLAY1".into(),
                // A panel remembered there but not plugged in, then the Dell.
                monitors: vec![(OLD.into(), false), (DELL.into(), true)],
            },
            AdapterMonitors {
                name: r"\\.\DISPLAY2".into(),
                monitors: vec![(LG.into(), true)],
            },
        ]
    }

    #[test]
    fn a_monitor_is_found_through_the_source_it_shows_and_its_rank_among_the_active_ones() {
        let hmonitors = [r"\\.\DISPLAY2".to_owned(), r"\\.\DISPLAY1".to_owned()];
        assert_eq!(
            locate(&adapters(), &hmonitors, DELL),
            Ok(Located {
                hmonitor: 1,
                physical: 0,
                expected: 1
            })
        );
        assert_eq!(
            locate(&adapters(), &hmonitors, &LG.to_lowercase()),
            Ok(Located {
                hmonitor: 0,
                physical: 0,
                expected: 1
            })
        );
    }

    #[test]
    fn a_duplicated_source_ranks_its_panels_in_order() {
        let adapters = vec![AdapterMonitors {
            name: r"\\.\DISPLAY1".into(),
            monitors: vec![(DELL.into(), true), (LG.into(), true)],
        }];
        let hmonitors = [r"\\.\DISPLAY1".to_owned()];
        let located = locate(&adapters, &hmonitors, LG).unwrap();
        assert_eq!((located.physical, located.expected), (1, 2));
    }

    #[test]
    fn a_monitor_off_the_desktop_or_unknown_has_no_handle() {
        let hmonitors = [r"\\.\DISPLAY1".to_owned(), r"\\.\DISPLAY2".to_owned()];
        assert!(locate(&adapters(), &hmonitors, OLD).is_err(), "inactive");
        assert!(locate(&adapters(), &hmonitors, "MONITOR\\XYZ0000\\x\\0009").is_err());
        // Its source has no HMONITOR: detached from the desktop.
        assert!(locate(&adapters(), &[r"\\.\DISPLAY2".to_owned()], DELL).is_err());
    }
}
