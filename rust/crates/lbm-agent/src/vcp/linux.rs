use super::{Monitor, Target, Write};
use lbm_ipc::vcp::{self, Choice, Feature};
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
                // A colour preset can change gain, brightness and contrast as well.
                let codes = if write.code == 0x14 {
                    state.features.iter().map(|f| f.code).collect::<Vec<_>>()
                } else {
                    vec![write.code]
                };
                let mut args = vec!["getvcp".into()];
                args.extend(codes.iter().map(|c| format!("{c:02x}")));
                args.push("--brief".into());
                let (_, output, _) = self.output(&bus, args).await?;
                let mut read = values(&output);
                let actual = *read.get(&write.code).ok_or("The command was sent, but its value could not be read back. Refresh the controls.")?;
                state.features.retain_mut(|f| {
                    if !codes.contains(&f.code) {
                        return true;
                    }
                    let Some((value, maximum)) = read.remove(&f.code) else {
                        return false;
                    };
                    f.value = value;
                    f.maximum = maximum;
                    !vcp::continuous(f.code) || (maximum > 0 && value <= maximum)
                });
                state.note = (actual.0 != write.value).then(|| {
                    format!(
                        "The monitor returned {} for {} (requested {}).",
                        actual.0,
                        vcp::name(write.code),
                        write.value
                    )
                });
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
            let Some((value, maximum)) = read.remove(&code) else {
                continue;
            };
            if vcp::continuous(code) && (maximum == 0 || value > maximum) {
                continue;
            }
            let choices = available.get(&code).cloned().unwrap_or_default();
            if !vcp::continuous(code) && choices.is_empty() {
                continue;
            }
            features.push(Feature {
                code,
                value,
                maximum,
                choices,
            });
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
        let mut cmd = tokio::process::Command::new(&self.tool);
        cmd.args(args)
            .args(["--bus", &bus.number.to_string()])
            .env("LC_ALL", "C")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(18), cmd.output())
            .await
            .map_err(|_| {
                "DDC/CI request timed out. Check the monitor's DDC/CI setting.".to_string()
            })?
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

fn serials(edid: &[u8]) -> Vec<String> {
    if edid.len() < 128 {
        return vec![];
    }
    let mut result = vec![];
    let numeric = u32::from_le_bytes(edid[12..16].try_into().unwrap());
    if numeric != 0 {
        result.push(numeric.to_string());
        result.push(format!("{numeric:08X}"));
    }
    for offset in [54, 72, 90, 108] {
        let d = &edid[offset..offset + 18];
        if d[..4] == [0, 0, 0, 0xff] {
            result.push(
                String::from_utf8_lossy(&d[5..18])
                    .trim_matches(['\0', '\n', '\r', ' '])
                    .to_string(),
            );
        }
    }
    result
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
    // Match the EDID serial even when only one connector remains: a monitor can
    // have been unplugged and replaced while the UI still holds the old layout.
    if let Some(serial) = &target.serial {
        found.retain(|b| serials(&b.edid).contains(serial));
    }
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
fn choice_label(code: u8, value: u16, reported: &str) -> String {
    match (code, value) {
        (0x8d, 1) => "Muted".into(),
        (0x8d, 2) => "Sound on".into(),
        (0xd6, 1) => "On".into(),
        (0xd6, 2) => "Standby".into(),
        (0xd6, 3) => "Suspend".into(),
        (0xd6, 4 | 5) => format!("Off (0x{value:02X})"),
        _ if reported.is_empty() || reported.starts_with("Unrecognized") => {
            format!("0x{value:02X}")
        }
        _ => reported.into(),
    }
}
/// Brief reads can include diagnostics or an unsupported code between valid lines.
fn values(output: &str) -> BTreeMap<u8, (u16, u16)> {
    output
        .lines()
        .filter_map(|line| {
            let t: Vec<_> = line.split_whitespace().collect();
            if t.first() != Some(&"VCP") {
                return None;
            }
            let code = u8::from_str_radix(t.get(1)?, 16).ok()?;
            match *t.get(2)? {
                "C" => Some((code, (t.get(3)?.parse().ok()?, t.get(4)?.parse().ok()?))),
                "CNC" if t.len() >= 7 => {
                    let byte = |i: usize| u8::from_str_radix(t[i].strip_prefix('x')?, 16).ok();
                    Some((
                        code,
                        (
                            u16::from_be_bytes([byte(5)?, byte(6)?]),
                            u16::from_be_bytes([byte(3)?, byte(4)?]),
                        ),
                    ))
                }
                "SNC" | "NC" => {
                    let v = t.get(3)?.strip_prefix('x')?;
                    Some((code, (u16::from_str_radix(v, 16).ok()?, 255)))
                }
                _ => None,
            }
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
        assert_eq!(read[&0x10], (9, 100));
        assert_eq!(read[&0x60], (15, 255));
        assert_eq!(read[&0x6c], (25, 50));
        assert!(!read.contains_key(&0x12));
    }
    fn connector(root: &Path, card: &str, serial: u32) {
        let p = root.join(format!("{card}-DP-1"));
        std::fs::create_dir_all(p.join("i2c-9")).unwrap();
        std::fs::write(p.join("status"), "connected\n").unwrap();
        let mut edid = vec![0; 128];
        edid[12..16].copy_from_slice(&serial.to_le_bytes());
        std::fs::write(p.join("edid"), edid).unwrap();
    }
    #[test]
    fn duplicate_connectors_and_replaced_monitors_cannot_pick_an_arbitrary_bus() {
        let root = tempfile::tempdir().unwrap();
        connector(root.path(), "card0", 42);
        let mut target = Target {
            connector: "DP-1".into(),
            serial: Some("43".into()),
        };
        assert!(find_bus(root.path(), &target).is_err());
        target.serial = Some("42".into());
        assert_eq!(find_bus(root.path(), &target).unwrap().number, 9);
        connector(root.path(), "card1", 43);
        target.serial = None;
        assert!(find_bus(root.path(), &target).is_err());
        target.serial = Some("43".into());
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
            serial: Some("42".into()),
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
}
