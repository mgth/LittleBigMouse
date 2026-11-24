use super::common::{choice_label, feature, Reading, PRESET, RGB};
use super::{DdcFuture, Monitor, Target, Write};
use lbm_ipc::vcp::{self, Choice};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Bus {
    path: PathBuf,
    number: u32,
    edid: Vec<u8>,
}

pub struct Service {
    tool: PathBuf,
    drm: PathBuf,
    next_token: u64,
    cached: Option<(Target, Bus, Monitor)>,
}
impl Default for Service {
    fn default() -> Self {
        Self {
            tool: "ddcutil".into(),
            drm: "/sys/class/drm".into(),
            next_token: 0,
            cached: None,
        }
    }
}
impl Service {
    pub async fn execute(
        &mut self,
        target: Target,
        write: Option<Write>,
    ) -> Result<Monitor, String> {
        // One total deadline, even when capabilities and several reads all fail.
        match tokio::time::timeout(Duration::from_secs(35), self.execute_inner(target, write)).await
        {
            Ok(result) => result,
            Err(_) => {
                self.cached = None;
                Err("The monitor did not respond in time. Check DDC/CI in its on-screen menu, then refresh.".into())
            }
        }
    }
    async fn execute_inner(
        &mut self,
        target: Target,
        write: Option<Write>,
    ) -> Result<Monitor, String> {
        let bus = find_bus(&self.drm, &target)?;
        if let Some(write) = write {
            let Some((old_target, old_bus, mut state)) = self.cached.clone() else {
                return Err("Refresh the monitor controls before changing a value.".into());
            };
            if old_target != target || old_bus != bus || state.token != write.token {
                return Err("The monitor changed; refresh its controls before editing.".into());
            }
            state
                .features
                .iter()
                .find(|f| f.code == write.code)
                .filter(|f| f.accepts(write.value))
                .ok_or("This value is not supported by the monitor.")?;
            // A failed or cancelled write can still have reached the monitor.
            self.cached = None;
            self.run(
                &bus,
                vec![
                    "setvcp".into(),
                    format!("{:02x}", write.code),
                    write.value.to_string(),
                    "--noverify".into(),
                ],
            )
            .await?;
            if matches!(write.code, 0x60 | 0xd6) {
                state
                    .features
                    .iter_mut()
                    .find(|f| f.code == write.code)
                    .unwrap()
                    .value = write.value;
                state.note = Some("Command sent. The monitor may stop answering after changing input or power; refresh when it is available again.".into());
            } else {
                // A colour preset can change gain, brightness and contrast as well, and
                // changing a gain can make the monitor leave its preset.
                let has_preset = state.features.iter().any(|f| f.code == PRESET);
                let codes = if write.code == PRESET {
                    state.features.iter().map(|f| f.code).collect::<Vec<_>>()
                } else if has_preset && RGB.contains(&write.code) {
                    vec![write.code, PRESET]
                } else {
                    vec![write.code]
                };
                let mut args = vec!["getvcp".into()];
                args.extend(codes.iter().map(|c| format!("{c:02x}")));
                args.push("--brief".into());
                let (_, output, _) = self.output(&bus, args).await?;
                let mut read = values(&output);
                let actual = *read.get(&write.code).ok_or("The command was sent, but its value could not be read back. Refresh the monitor controls.")?;
                // What could not be read back is unknown now, and showing its old value
                // would make selecting that value again do nothing.
                let before = state.features.len();
                state.features = std::mem::take(&mut state.features)
                    .into_iter()
                    .filter_map(|f| {
                        if !codes.contains(&f.code) {
                            return Some(f);
                        }
                        feature(f.code, read.remove(&f.code)?, f.choices)
                    })
                    .collect();
                let lost = state.features.len() < before;
                state.note = (actual.value != write.value).then(|| {
                    format!(
                        "The monitor returned {} for {} (requested {}).",
                        actual.value,
                        vcp::name(write.code),
                        write.value
                    )
                });
                if lost && state.note.is_none() {
                    state.note =
                        Some("Some controls could not be read back. Refresh to show them.".into());
                }
            }
            self.cached = Some((target, bus, state.clone()));
            return Ok(state);
        }
        self.cached = None;
        let caps = self.run(&bus, vec!["capabilities".into()]).await;
        let available = caps.as_ref().map(|s| capabilities(s)).unwrap_or_default();
        let fallback = available.is_empty();
        let codes: Vec<_> = if fallback {
            vec![0x10, 0x12, 0x16, 0x18, 0x1a]
        } else {
            vcp::CODES
                .iter()
                .copied()
                .filter(|c| available.contains_key(c))
                .collect()
        };
        if codes.is_empty() {
            return Err("This monitor advertises none of the supported VCP controls.".into());
        }
        let mut args = vec!["getvcp".into()];
        args.extend(codes.iter().map(|c| format!("{c:02x}")));
        args.push("--brief".into());
        let (ok, output, error) = self.output(&bus, args).await?;
        let mut read = values(&output);
        if read.is_empty() {
            return Err(format!("No DDC/CI response from {}. Enable DDC/CI in the monitor menu and check the cable or dock. {}", target.connector, diagnostic(&error, &output)));
        }
        let mut features = vec![];
        for code in codes {
            let Some(reading) = read.remove(&code) else {
                continue;
            };
            let choices = available.get(&code).cloned().unwrap_or_default();
            if !vcp::continuous(code) && choices.is_empty() {
                continue;
            }
            features.extend(feature(code, reading, choices));
        }
        if features.is_empty() {
            return Err("The monitor returned no usable controls.".into());
        }
        self.next_token += 1;
        let state = Monitor {
            token: self.next_token,
            connection: format!("{} · DDC/CI · I²C {}", target.connector, bus.number),
            features,
            note: if fallback {
                Some("Capabilities unavailable; showing controls that answered the probe.".into())
            } else if !ok {
                Some(
                    "Some advertised controls did not answer; only readable controls are shown."
                        .into(),
                )
            } else {
                None
            },
        };
        self.cached = Some((target, bus, state.clone()));
        Ok(state)
    }
    async fn run(&self, bus: &Bus, args: Vec<String>) -> Result<String, String> {
        let (ok, stdout, stderr) = self.output(bus, args).await?;
        if ok {
            Ok(stdout)
        } else {
            Err(diagnostic(&stderr, &stdout))
        }
    }
    async fn output(&self, bus: &Bus, args: Vec<String>) -> Result<(bool, String, String), String> {
        ddcutil(&self.tool, bus.number, args).await
    }

    /// What this service read of a monitor is no longer to be trusted: the input
    /// switching wrote to it behind the property panel's back.
    pub fn forget(&mut self) {
        self.cached = None;
    }
}

async fn ddcutil(
    tool: &Path,
    bus: u32,
    args: Vec<String>,
) -> Result<(bool, String, String), String> {
    let mut cmd = tokio::process::Command::new(tool);
    cmd.args(args)
        .args(["--bus", &bus.to_string()])
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(18), cmd.output())
        .await
        .map_err(|_| "DDC/CI request timed out. Check the monitor's DDC/CI setting.".to_string())?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "ddcutil is not installed. Install it to control monitor hardware.".into()
            } else {
                format!("Cannot start ddcutil: {e}")
            }
        })?;
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into(),
        String::from_utf8_lossy(&output.stderr).into(),
    ))
}

/// One VCP code read or written on its own, through ddcutil: what the input switching
/// needs, without the property panel's probe and tokens.
pub struct SystemDdc {
    tool: PathBuf,
    drm: PathBuf,
}

impl Default for SystemDdc {
    fn default() -> Self {
        Self {
            tool: "ddcutil".into(),
            drm: "/sys/class/drm".into(),
        }
    }
}

impl super::Ddc for SystemDdc {
    fn read(&self, target: Target, code: u8) -> DdcFuture<u16> {
        let (tool, drm) = (self.tool.clone(), self.drm.clone());
        Box::pin(async move {
            let bus = find_bus(&drm, &target)?;
            let args = vec!["getvcp".into(), format!("{code:02x}"), "--brief".into()];
            let (_, output, error) = ddcutil(&tool, bus.number, args).await?;
            values(&output)
                .remove(&code)
                .map(|reading| reading.value)
                .ok_or_else(|| {
                    format!(
                        "no DDC/CI answer for {} from {}: {}",
                        vcp::name(code),
                        target.connector,
                        diagnostic(&error, &output)
                    )
                })
        })
    }

    fn write(&self, target: Target, code: u8, value: u16) -> DdcFuture<()> {
        let (tool, drm) = (self.tool.clone(), self.drm.clone());
        Box::pin(async move {
            let bus = find_bus(&drm, &target)?;
            // Not verified: a monitor that changed input may well stop answering on
            // this cable, which is no failure of the write.
            let args = vec![
                "setvcp".into(),
                format!("{code:02x}"),
                value.to_string(),
                "--noverify".into(),
            ];
            let (ok, output, error) = ddcutil(&tool, bus.number, args).await?;
            if ok {
                Ok(())
            } else {
                Err(diagnostic(&error, &output))
            }
        })
    }
}
fn diagnostic(stderr: &str, stdout: &str) -> String {
    let message = if stderr.trim().is_empty() {
        stdout
    } else {
        stderr
    };
    let detail: String = message.trim().chars().take(700).collect();
    if detail.to_lowercase().contains("permission") {
        format!("No permission to access the monitor's I²C device. Check the ddcutil udev permissions. {detail}")
    } else if detail.is_empty() {
        "The monitor rejected the DDC/CI request.".into()
    } else {
        detail
    }
}

/// Whether this EDID is the monitor the layout knows, read the way the layout reads it.
fn is_target(edid: &[u8], target: &Target) -> bool {
    let detected = lbm_display::edid::parse("", edid).to_linux();
    target
        .serial
        .as_deref()
        .is_none_or(|serial| detected.monitor_serial() == Some(serial))
        && target
            .model
            .as_ref()
            .is_none_or(|model| detected.pnp_code().as_ref() == Some(model))
}
fn find_bus(root: &Path, target: &Target) -> Result<Bus, String> {
    let mut found = vec![];
    for entry in std::fs::read_dir(root)
        .map_err(|e| format!("Cannot read DRM connectors: {e}"))?
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((card, connector)) = name.split_once('-') else {
            continue;
        };
        if !card
            .strip_prefix("card")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            || connector != target.connector
        {
            continue;
        }
        let path = entry.path();
        if std::fs::read_to_string(path.join("status"))
            .unwrap_or_default()
            .trim()
            != "connected"
        {
            continue;
        }
        let edid = std::fs::read(path.join("edid")).unwrap_or_default();
        if edid.len() < 128 {
            continue;
        }
        let adapters: Vec<_> = std::fs::read_dir(&path)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_prefix("i2c-")?
                    .parse::<u32>()
                    .ok()
            })
            .collect();
        let number = if adapters.len() == 1 {
            Some(adapters[0])
        } else if adapters.is_empty() {
            std::fs::canonicalize(path.join("ddc"))
                .ok()
                .and_then(|p| p.file_name()?.to_str()?.strip_prefix("i2c-")?.parse().ok())
        } else {
            None
        };
        if let Some(number) = number {
            found.push(Bus { path, number, edid });
        }
    }
    // Match the EDID even when only one connector remains: a monitor can have been
    // unplugged and replaced while the UI still holds the old layout.
    found.retain(|b| is_target(&b.edid, target));
    if found.len() != 1 {
        return Err(format!("Cannot uniquely identify the DDC/CI adapter for {}. Refresh the display layout and check that the monitor is connected.", target.connector));
    }
    Ok(found.remove(0))
}

/// ddcutil's English capabilities listing: continuous features have no Values;
/// enumerations can use either `01: label` or `Values: 01 02 (...)`.
fn capabilities(output: &str) -> BTreeMap<u8, Vec<Choice>> {
    let mut result = BTreeMap::<u8, Vec<Choice>>::new();
    let mut code = None;
    for line in output.lines().map(str::trim) {
        if let Some(feature) = line.strip_prefix("Feature:") {
            code = feature
                .split_whitespace()
                .next()
                .and_then(|v| u8::from_str_radix(v, 16).ok());
            if let Some(c) = code {
                result.entry(c).or_default();
            }
        } else if let Some(c) = code {
            let choices = result.get_mut(&c).unwrap();
            if let Some(values) = line.strip_prefix("Values:") {
                for token in values.split_whitespace() {
                    let Ok(value) = u16::from_str_radix(token, 16) else {
                        break;
                    };
                    choices.push(Choice {
                        value,
                        label: choice_label(c, value, ""),
                    });
                }
            } else if let Some((value, label)) = line.split_once(':') {
                if value.len() == 2 {
                    if let Ok(value) = u16::from_str_radix(value, 16) {
                        choices.push(Choice {
                            value,
                            label: choice_label(c, value, label.trim()),
                        });
                    }
                }
            }
        }
    }
    result
}
/// Brief reads can include diagnostics or an unsupported code between valid lines.
fn values(output: &str) -> BTreeMap<u8, Reading> {
    output
        .lines()
        .filter_map(|line| {
            let t: Vec<_> = line.split_whitespace().collect();
            if t.first() != Some(&"VCP") {
                return None;
            }
            let code = u8::from_str_radix(t.get(1)?, 16).ok()?;
            let reading = match *t.get(2)? {
                "C" => Reading {
                    value: t.get(3)?.parse().ok()?,
                    maximum: t.get(4)?.parse().ok()?,
                    plain: true,
                },
                "CNC" if t.len() >= 7 => {
                    let byte = |i: usize| u8::from_str_radix(t[i].strip_prefix('x')?, 16).ok();
                    let (high, low) = (byte(5)?, byte(6)?);
                    // Only a true continuous value spans both bytes. Elsewhere, as for
                    // mute, SH is a separate field and the state is SL.
                    let whole = vcp::continuous(code) && code != vcp::VOLUME;
                    Reading {
                        value: if whole {
                            u16::from_be_bytes([high, low])
                        } else {
                            low.into()
                        },
                        maximum: u16::from_be_bytes([byte(3)?, byte(4)?]),
                        plain: false,
                    }
                }
                "SNC" | "NC" => Reading {
                    value: u16::from_str_radix(t.get(3)?.strip_prefix('x')?, 16).ok()?,
                    maximum: 255,
                    plain: false,
                },
                _ => return None,
            };
            Some((code, reading))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_real_capabilities_and_partial_reads_without_inventing_ranges() {
        let caps = capabilities("Feature: 10 (Luminosity)\nFeature: 60 (Input Source)\n Values:\n 0f: DisplayPort-1\n 11: HDMI-1\nFeature: 8D (Audio Mute)\n Values: 01 02 (interpretation unavailable)\n");
        assert!(caps[&0x10].is_empty());
        assert_eq!(caps[&0x60][0].value, 15);
        assert_eq!(caps[&0x8d][1].label, "Sound on");
        let read =
            values("diagnostic\nVCP 10 C 9 100\nVCP 12 ERR\nVCP 60 SNC x0f\nVCP 6C C 25 50\n");
        assert_eq!((read[&0x10].value, read[&0x10].maximum), (9, 100));
        assert_eq!((read[&0x60].value, read[&0x60].maximum), (15, 255));
        assert_eq!((read[&0x6c].value, read[&0x6c].maximum), (25, 50));
        assert!(!read.contains_key(&0x12));
    }
    fn edid(serial: u32) -> Vec<u8> {
        let mut edid = vec![0; 128];
        edid[12..16].copy_from_slice(&serial.to_le_bytes());
        edid
    }
    fn connector_with(root: &Path, card: &str, edid: &[u8]) {
        let p = root.join(format!("{card}-DP-1"));
        std::fs::create_dir_all(p.join("i2c-9")).unwrap();
        std::fs::write(p.join("status"), "connected\n").unwrap();
        std::fs::write(p.join("edid"), edid).unwrap();
    }
    fn connector(root: &Path, card: &str, serial: u32) {
        connector_with(root, card, &edid(serial));
    }
    /// The target of a monitor detected with this EDID, through the real layout.
    fn target_of(edid: Option<&[u8]>) -> Target {
        let mut layout = lbm_layout::model::Layout::new(lbm_layout::model::LayoutOptions {
            vcp_control: true,
            ..Default::default()
        });
        lbm_layout::linux::add_monitor(
            &mut layout,
            &lbm_layout::linux::LinuxMonitor {
                connector_name: "DP-1".into(),
                edid: edid.map(|e| lbm_display::edid::parse("DP-1", e).to_linux()),
                ..lbm_layout::linux::LinuxMonitor::fallback()
            },
        );
        let id = layout.monitors()[0].id.clone();
        crate::vcp::target(&layout, &id).unwrap()
    }
    #[test]
    fn the_bus_is_matched_with_the_serial_the_layout_read_from_the_same_edid() {
        let mut spaced = edid(0);
        spaced[54..58].copy_from_slice(&[0, 0, 0, 0xff]);
        spaced[59..69].copy_from_slice(b"ABC123   \n");
        for bytes in [edid(42), edid(0), spaced] {
            let root = tempfile::tempdir().unwrap();
            connector_with(root.path(), "card0", &bytes);
            let target = target_of(Some(&bytes));
            assert_eq!(find_bus(root.path(), &target).unwrap().number, 9);
            // The same connector with another monitor behind it is refused.
            let root = tempfile::tempdir().unwrap();
            connector(root.path(), "card0", 7);
            assert!(find_bus(root.path(), &target).is_err(), "{target:?}");
        }
    }
    #[test]
    fn another_model_with_the_same_serial_is_not_the_monitor_of_the_layout() {
        let product = |code| {
            let mut bytes = edid(0);
            bytes[8..10].copy_from_slice(&[0x10, 0xac]);
            bytes[10] = code;
            bytes
        };
        let target = target_of(Some(&product(1)));
        assert!(target.model.is_some());
        let root = tempfile::tempdir().unwrap();
        connector_with(root.path(), "card0", &product(2));
        assert!(find_bus(root.path(), &target).is_err());
        let root = tempfile::tempdir().unwrap();
        connector_with(root.path(), "card0", &product(1));
        assert_eq!(find_bus(root.path(), &target).unwrap().number, 9);
    }
    #[test]
    fn structured_replies_keep_their_fields_apart() {
        // MCCS 2.2 mute: SH says the picture is on, SL that the sound is off.
        let read = values("VCP 8D CNC x00 x00 x02 x01\nVCP 10 CNC x01 x00 x00 xfa\n");
        assert_eq!(read[&0x8d].value, 1);
        assert_eq!((read[&0x10].value, read[&0x10].maximum), (250, 256));
    }
    #[test]
    fn only_a_plain_volume_is_a_level_from_zero() {
        let structured = |value| Reading {
            value,
            maximum: 255,
            plain: false,
        };
        let volume = feature(vcp::VOLUME, structured(40), vec![]).unwrap();
        assert_eq!((volume.minimum, volume.maximum), (1, 254));
        assert!(!volume.choices.is_empty());
        for special in [0, 255] {
            let volume = feature(vcp::VOLUME, structured(special), vec![]).unwrap();
            assert!(volume.accepts(special), "{special}");
            assert_eq!(volume.value, special);
        }
        let plain = Reading {
            value: 0,
            maximum: 100,
            plain: true,
        };
        let volume = feature(vcp::VOLUME, plain, vec![]).unwrap();
        assert_eq!((volume.minimum, volume.maximum), (0, 100));
        assert!(volume.choices.is_empty());
        // MCCS 2.2a gives the levels directly in SL, with no maximum in the reply.
        let read = values("VCP 62 CNC x00 x00 x00 x20\n");
        let volume = feature(vcp::VOLUME, read[&vcp::VOLUME], vec![]).unwrap();
        assert_eq!((volume.value, volume.minimum, volume.maximum), (32, 1, 254));
    }
    #[tokio::test]
    async fn a_preset_that_cannot_be_read_back_is_no_longer_shown_as_current() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        connector(dir.path(), "card0", 42);
        let executable = dir.path().join("ddcutil-test");
        let written = dir.path().join("written");
        let script = format!(
            r#"#!/bin/sh
case "$1" in
capabilities) printf 'Feature: 14 (Color Preset)\n Values:\n 05: sRGB\n 0b: User 1\nFeature: 16 (Red)\n';;
getvcp) if [ -e '{0}' ]; then printf 'VCP 14 ERR\n'; else printf 'VCP 14 SNC x05\n'; fi
        printf 'VCP 16 C 50 100\n';;
setvcp) : > '{0}';;
esac
"#,
            written.display()
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut service = Service {
            tool: executable,
            drm: dir.path().into(),
            ..Default::default()
        };
        let target = Target {
            connector: "DP-1".into(),
            serial: None,
            model: None,
        };
        let state = service.execute(target.clone(), None).await.unwrap();
        assert!(state.features.iter().any(|f| f.code == PRESET));
        let write = Write {
            token: state.token,
            code: 0x16,
            value: 50,
        };
        let state = service.execute(target, Some(write)).await.unwrap();
        assert!(state.features.iter().all(|f| f.code != PRESET));
        assert!(state.features.iter().any(|f| f.code == 0x16));
        assert!(state.note.is_some());
    }
    #[tokio::test]
    async fn a_gain_change_also_reads_back_the_preset_it_may_have_left() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        connector(dir.path(), "card0", 42);
        let executable = dir.path().join("ddcutil-test");
        let preset = dir.path().join("preset");
        std::fs::write(&preset, "05").unwrap();
        let script = format!(
            r#"#!/bin/sh
case "$1" in
capabilities) printf 'Feature: 14 (Color Preset)\n Values:\n 05: sRGB\n 0b: User 1\nFeature: 16 (Red)\n';;
getvcp) printf 'VCP 14 SNC x%s\nVCP 16 C 50 100\n' "$(cat '{0}')";;
setvcp) printf 0b > '{0}';;
esac
"#,
            preset.display()
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut service = Service {
            tool: executable,
            drm: dir.path().into(),
            ..Default::default()
        };
        let target = Target {
            connector: "DP-1".into(),
            serial: None,
            model: None,
        };
        let state = service.execute(target.clone(), None).await.unwrap();
        let preset_value =
            |m: &Monitor| m.features.iter().find(|f| f.code == PRESET).unwrap().value;
        assert_eq!(preset_value(&state), 5);
        let write = Write {
            token: state.token,
            code: 0x16,
            value: 50,
        };
        let state = service.execute(target, Some(write)).await.unwrap();
        assert_eq!(preset_value(&state), 0x0b);
    }
    #[test]
    fn a_monitor_detected_without_an_edid_has_no_serial_to_compare() {
        let target = target_of(None);
        assert_eq!(target.serial, None);
        let root = tempfile::tempdir().unwrap();
        connector(root.path(), "card0", 42);
        assert_eq!(find_bus(root.path(), &target).unwrap().number, 9);
    }
    #[test]
    fn duplicate_connectors_and_replaced_monitors_cannot_pick_an_arbitrary_bus() {
        let root = tempfile::tempdir().unwrap();
        connector(root.path(), "card0", 42);
        let mut target = Target {
            connector: "DP-1".into(),
            serial: Some("0000002B".into()),
            model: None,
        };
        assert!(find_bus(root.path(), &target).is_err());
        target.serial = Some("0000002A".into());
        assert_eq!(find_bus(root.path(), &target).unwrap().number, 9);
        connector(root.path(), "card1", 43);
        target.serial = None;
        assert!(find_bus(root.path(), &target).is_err());
        target.serial = Some("0000002B".into());
        assert!(find_bus(root.path(), &target)
            .unwrap()
            .path
            .ends_with("card1-DP-1"));
    }
    #[tokio::test]
    async fn writes_are_checked_read_back_and_never_sent_to_a_replacement_monitor() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        connector(dir.path(), "card0", 42);
        let executable = dir.path().join("ddcutil-test");
        let log = dir.path().join("commands");
        let value = dir.path().join("value");
        std::fs::write(&value, "30").unwrap();
        let script = format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{}'
case "$1" in
capabilities) printf 'Feature: 10 (Brightness)\nFeature: 62 (Volume)\n';;
getvcp) printf 'VCP 10 C %s 80\nVCP 62 CNC x00 x64 x00 x11\n' "$(cat '{}')";;
setvcp) printf '%s' "$3" > '{}';;
esac
"#,
            log.display(),
            value.display(),
            value.display()
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut service = Service {
            tool: executable,
            drm: dir.path().into(),
            ..Default::default()
        };
        let target = Target {
            connector: "DP-1".into(),
            serial: Some("0000002A".into()),
            model: None,
        };
        let state = service.execute(target.clone(), None).await.unwrap();
        assert_eq!(state.features[0].maximum, 80);
        assert_eq!(
            (state.features[1].value, state.features[1].maximum),
            (17, 100)
        );
        let lines = || std::fs::read_to_string(&log).unwrap().lines().count();
        let before = lines();
        for write in [
            Write {
                token: state.token,
                code: 0x10,
                value: 81,
            },
            Write {
                token: state.token,
                code: 0x04,
                value: 1,
            },
            Write {
                token: state.token + 1,
                code: 0x10,
                value: 40,
            },
        ] {
            assert!(service.execute(target.clone(), Some(write)).await.is_err());
        }
        assert_eq!(lines(), before);
        let new = service
            .execute(
                target.clone(),
                Some(Write {
                    token: state.token,
                    code: 0x10,
                    value: 41,
                }),
            )
            .await
            .unwrap();
        assert_eq!(new.features[0].value, 41);
        assert!(std::fs::read_to_string(&log)
            .unwrap()
            .contains("setvcp 10 41 --noverify --bus 9"));
        let before = lines();
        connector(dir.path(), "card0", 43);
        assert!(service
            .execute(
                target,
                Some(Write {
                    token: state.token,
                    code: 0x10,
                    value: 42
                })
            )
            .await
            .is_err());
        assert_eq!(lines(), before);
    }

    #[tokio::test]
    async fn the_input_is_read_and_written_on_its_own_without_a_probe() {
        use super::super::Ddc;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        connector(dir.path(), "card0", 42);
        let executable = dir.path().join("ddcutil-test");
        let log = dir.path().join("log");
        let script = format!(
            r#"#!/bin/sh
echo "$@" >> '{0}'
case "$1" in
getvcp) printf 'VCP 60 SNC x11\n';;
setvcp) ;;
*) exit 1;;
esac
"#,
            log.display()
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let ddc = SystemDdc {
            tool: executable,
            drm: dir.path().into(),
        };
        let target = Target {
            connector: "DP-1".into(),
            serial: None,
            model: None,
        };
        assert_eq!(ddc.read(target.clone(), 0x60).await, Ok(0x11));
        ddc.write(target, 0x60, 0x0f).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&log).unwrap(),
            "getvcp 60 --brief --bus 9\nsetvcp 60 15 --noverify --bus 9\n"
        );
        // Another monitor on that connector is not written to.
        let other = Target {
            connector: "DP-1".into(),
            serial: Some("nope".into()),
            model: None,
        };
        assert!(ddc.write(other, 0x60, 0x0f).await.is_err());
    }
}
