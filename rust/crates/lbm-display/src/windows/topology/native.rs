//! One CCD transaction: test the complete configuration, then apply it once.
use super::{Backend, Configuration, Id, Path, PathId, Plan, Source};
use std::{collections::HashMap, fmt::Write, mem::size_of, ptr::addr_of_mut};
use windows::Win32::{
    Devices::Display::*,
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LUID},
    Graphics::Gdi::{DISPLAYCONFIG_PATH_ACTIVE, DISPLAYCONFIG_PATH_MODE_IDX_INVALID},
};

#[derive(Default)]
pub(super) struct Native {
    paths: Vec<DISPLAYCONFIG_PATH_INFO>,
    modes: Vec<DISPLAYCONFIG_MODE_INFO>,
}
fn id(adapter: LUID, id: u32) -> Id {
    Id {
        adapter: (u64::from(adapter.HighPart as u32) << 32) | u64::from(adapter.LowPart),
        id,
    }
}
fn path_id(p: &DISPLAYCONFIG_PATH_INFO) -> PathId {
    PathId {
        source: id(p.sourceInfo.adapterId, p.sourceInfo.id),
        target: id(p.targetInfo.adapterId, p.targetInfo.id),
    }
}
fn text(buffer: &[u16]) -> String {
    String::from_utf16_lossy(&buffer[..buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len())])
}
fn error(api: &str, status: i32) -> String {
    format!("{api} failed (Windows error {status}). Refresh the output list; a local interactive Windows session with a WDDM display driver is required.")
}
fn target_name(adapter: LUID, id: u32) -> Result<(String, String), String> {
    let mut name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: adapter,
            id,
        },
        ..Default::default()
    };
    // SAFETY: the pointer addresses the complete initialized packet described by its header.
    let status = unsafe { DisplayConfigGetDeviceInfo(addr_of_mut!(name).cast()) };
    if status != 0 {
        return Err(error("DisplayConfigGetDeviceInfo", status));
    }
    Ok((
        text(&name.monitorDevicePath),
        text(&name.monitorFriendlyDeviceName),
    ))
}
impl Native {
    fn read(&mut self) -> Result<(), String> {
        for _ in 0..3 {
            let mut np = 0;
            let mut nm = 0;
            // SAFETY: both counts are writable; ALL_PATHS discovers detached targets too.
            let status = unsafe { GetDisplayConfigBufferSizes(QDC_ALL_PATHS, &mut np, &mut nm) };
            if status != ERROR_SUCCESS {
                return Err(error("GetDisplayConfigBufferSizes", status.0 as i32));
            }
            let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np.max(1) as usize];
            let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm.max(1) as usize];
            // SAFETY: buffers hold the supplied counts; ALL_PATHS requires a null topology id.
            // No VIRTUAL_MODE_AWARE flag: the unions contain plain mode indices.
            let status = unsafe {
                QueryDisplayConfig(
                    QDC_ALL_PATHS,
                    &mut np,
                    paths.as_mut_ptr(),
                    &mut nm,
                    modes.as_mut_ptr(),
                    None,
                )
            };
            if status == ERROR_INSUFFICIENT_BUFFER {
                continue;
            }
            if status != ERROR_SUCCESS {
                return Err(error("QueryDisplayConfig", status.0 as i32));
            }
            paths.truncate(np as usize);
            modes.truncate(nm as usize);
            self.paths = paths;
            self.modes = modes;
            return Ok(());
        }
        Err(
            "The display configuration kept changing during enumeration; refresh the output list."
                .into(),
        )
    }
    fn configuration(&self) -> Result<Configuration, String> {
        let mut result = Configuration::default();
        let mut names = HashMap::new();
        for p in &self.paths {
            let key = path_id(p);
            let active = p.flags & DISPLAYCONFIG_PATH_ACTIVE != 0;
            let available = p.targetInfo.targetAvailable.as_bool();
            let (connector, name) = if available {
                if let std::collections::hash_map::Entry::Vacant(entry) = names.entry(key.target) {
                    entry.insert(target_name(p.targetInfo.adapterId, p.targetInfo.id)?);
                }
                names[&key.target].clone()
            } else {
                (String::new(), String::new())
            };
            result.paths.push(Path {
                id: key,
                connector,
                name,
                active,
                available,
            });
            // SAFETY: the non-virtual query above returned the modeInfoIdx union members.
            let (source_index, target_index) = unsafe {
                (
                    p.sourceInfo.Anonymous.modeInfoIdx,
                    p.targetInfo.Anonymous.modeInfoIdx,
                )
            };
            let t = &p.targetInfo;
            let _ = write!(
                result.details,
                "{:?};{source_index};{target_index};{};{};{};{};{};{:?};{};{}|",
                key,
                p.flags,
                p.sourceInfo.statusFlags,
                t.outputTechnology.0,
                t.rotation.0,
                t.scaling.0,
                t.refreshRate,
                t.scanLineOrdering.0,
                t.statusFlags
            );
            if active {
                let mode = self
                    .modes
                    .get(source_index as usize)
                    .ok_or("Windows returned an invalid active source mode index.")?;
                if mode.infoType != DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
                    || id(mode.adapterId, mode.id) != key.source
                {
                    return Err("Windows returned a mismatched active source mode.".into());
                }
                if !result.sources.iter().any(|s| s.id == key.source) {
                    // SAFETY: infoType was checked immediately above.
                    let mode = unsafe { mode.Anonymous.sourceMode };
                    result.sources.push(Source {
                        id: key.source,
                        x: mode.position.x,
                        y: mode.position.y,
                        width: mode.width,
                        height: mode.height,
                        pixel_format: mode.pixelFormat.0,
                    });
                }
            }
        }
        for mode in &self.modes {
            let _ = write!(
                result.details,
                "{:?}:{}:",
                id(mode.adapterId, mode.id),
                mode.infoType.0
            );
            if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE {
                // SAFETY: the tag selects this initialized union member.
                let source = unsafe { mode.Anonymous.sourceMode };
                let _ = write!(result.details, "{source:?}|");
            } else if mode.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_TARGET {
                // SAFETY: the tag selects targetMode; the nested union is a u32 bitfield.
                let signal = unsafe { mode.Anonymous.targetMode.targetVideoSignalInfo };
                let standard = unsafe { signal.Anonymous.videoStandard };
                let _ = write!(
                    result.details,
                    "{}:{:?}:{:?}:{:?}:{:?}:{}:{}|",
                    signal.pixelRate,
                    signal.hSyncFreq,
                    signal.vSyncFreq,
                    signal.activeSize,
                    signal.totalSize,
                    standard,
                    signal.scanLineOrdering.0
                );
            }
        }
        // Include effective DPI changes, which do not necessarily alter CCD pixel modes.
        result
            .details
            .push_str(&super::super::current_display_signature());
        Ok(result)
    }
    fn materialize(
        &self,
        plan: &Plan,
    ) -> Result<(Vec<DISPLAYCONFIG_PATH_INFO>, Vec<DISPLAYCONFIG_MODE_INFO>), String> {
        let mut paths = Vec::new();
        for key in &plan.paths {
            let matching: Vec<_> = self.paths.iter().filter(|p| path_id(p) == *key).collect();
            if matching.len() != 1 {
                return Err(
                    "The Windows path is missing or ambiguous; refresh the output list.".into(),
                );
            }
            let mut path = *matching[0];
            path.flags |= DISPLAYCONFIG_PATH_ACTIVE;
            if plan.activate == Some(*key) {
                path.sourceInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
                path.targetInfo.Anonymous.modeInfoIdx = DISPLAYCONFIG_PATH_MODE_IDX_INVALID;
            }
            paths.push(path);
        }
        let mut modes = self.modes.clone();
        for (key, x, y) in &plan.positions {
            let mode = modes
                .iter_mut()
                .find(|m| {
                    m.infoType == DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE
                        && id(m.adapterId, m.id) == *key
                })
                .ok_or("The Windows source mode disappeared.")?;
            // Only a source mode is selected; the remaining mode fields are preserved.
            mode.Anonymous.sourceMode.position.x = *x;
            mode.Anonymous.sourceMode.position.y = *y;
        }
        Ok((paths, modes))
    }
    fn submit(&self, plan: &Plan, apply: bool) -> Result<(), String> {
        let (paths, modes) = self.materialize(plan)?;
        let mut flags = SDC_USE_SUPPLIED_DISPLAY_CONFIG;
        flags |= if apply {
            SDC_APPLY | SDC_SAVE_TO_DATABASE
        } else {
            SDC_VALIDATE
        };
        if plan.allow_changes {
            flags |= SDC_ALLOW_CHANGES;
        }
        // SAFETY: both arrays come from the same successful CCD query; mode indices are
        // retained, except the explicitly activated path whose mode selection is requested.
        let status = unsafe { SetDisplayConfig(Some(&paths), Some(&modes), flags) };
        if status != 0 {
            return Err(error(
                if apply {
                    "SetDisplayConfig(apply)"
                } else {
                    "SetDisplayConfig(validate)"
                },
                status,
            ));
        }
        Ok(())
    }
}
impl Backend for Native {
    fn query(&mut self) -> Result<Configuration, String> {
        self.read()?;
        self.configuration()
    }
    fn validate(&mut self, plan: &Plan) -> Result<(), String> {
        self.submit(plan, false)
    }
    fn apply(&mut self, plan: &Plan) -> Result<(), String> {
        self.submit(plan, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn native() -> Native {
        let adapter = LUID {
            LowPart: 1,
            HighPart: 0,
        };
        let mut source = DISPLAYCONFIG_MODE_INFO {
            infoType: DISPLAYCONFIG_MODE_INFO_TYPE_SOURCE,
            id: 0,
            adapterId: adapter,
            ..Default::default()
        };
        source.Anonymous.sourceMode = DISPLAYCONFIG_SOURCE_MODE {
            width: 1920,
            height: 1080,
            pixelFormat: DISPLAYCONFIG_PIXELFORMAT_32BPP,
            position: windows::Win32::Foundation::POINTL { x: 100, y: 200 },
        };
        let mut path = DISPLAYCONFIG_PATH_INFO::default();
        path.sourceInfo.adapterId = adapter;
        path.sourceInfo.id = 0;
        path.sourceInfo.Anonymous.modeInfoIdx = 0;
        path.targetInfo.adapterId = adapter;
        path.targetInfo.id = 9;
        path.targetInfo.Anonymous.modeInfoIdx = 1;
        path.targetInfo.rotation = DISPLAYCONFIG_ROTATION_ROTATE90;
        path.flags = DISPLAYCONFIG_PATH_ACTIVE;
        Native {
            paths: vec![path],
            modes: vec![
                source,
                DISPLAYCONFIG_MODE_INFO {
                    infoType: DISPLAYCONFIG_MODE_INFO_TYPE_TARGET,
                    id: 9,
                    adapterId: adapter,
                    ..Default::default()
                },
            ],
        }
    }
    #[test]
    fn a_primary_change_only_translates_the_source_and_preserves_target_modes() {
        let native = native();
        let key = path_id(&native.paths[0]);
        let plan = Plan {
            paths: vec![key],
            activate: None,
            positions: vec![(key.source, 0, 0)],
            allow_changes: false,
        };
        let (paths, modes) = native.materialize(&plan).unwrap();
        // SAFETY: fixture uses the non-virtual source/target indices and a source mode tag.
        unsafe {
            assert_eq!(paths[0].sourceInfo.Anonymous.modeInfoIdx, 0);
            assert_eq!(paths[0].targetInfo.Anonymous.modeInfoIdx, 1);
            assert_eq!(modes[0].Anonymous.sourceMode.position.x, 0);
            assert_eq!(modes[0].Anonymous.sourceMode.width, 1920);
            assert_eq!(native.modes[0].Anonymous.sourceMode.position.x, 100);
        }
        assert_eq!(
            paths[0].targetInfo.rotation,
            DISPLAYCONFIG_ROTATION_ROTATE90
        );
    }
    #[test]
    fn attaching_invalidates_both_mode_indices_of_the_exact_target() {
        let native = native();
        let key = path_id(&native.paths[0]);
        let plan = Plan {
            paths: vec![key],
            activate: Some(key),
            positions: vec![],
            allow_changes: true,
        };
        let (paths, _) = native.materialize(&plan).unwrap();
        assert_eq!(path_id(&paths[0]), key);
        // SAFETY: materialize initializes both non-virtual index union members.
        unsafe {
            assert_eq!(
                paths[0].sourceInfo.Anonymous.modeInfoIdx,
                DISPLAYCONFIG_PATH_MODE_IDX_INVALID
            );
            assert_eq!(
                paths[0].targetInfo.Anonymous.modeInfoIdx,
                DISPLAYCONFIG_PATH_MODE_IDX_INVALID
            );
        }
    }
}
