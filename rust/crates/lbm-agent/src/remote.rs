//! Native Samsung WebSocket and Hisense VIDAA MQTT remotes. No foreign runtime.
mod hisense;
mod mqtt;
mod network;
mod samsung;
mod store;
use lbm_ipc::remote::{Action, Brand, Command, State};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use store::{Saved, Store};

pub struct Service {
    store: Store,
    samsung: HashMap<String, samsung::Client>,
    hisense: HashMap<String, mqtt::Client>,
    states: HashMap<String, State>,
}
impl Default for Service {
    fn default() -> Self {
        Self::new(lbm_store::lbm_paths::config_dir())
    }
}
impl Service {
    pub fn new(path: PathBuf) -> Self {
        Self {
            store: Store::new(path),
            samsung: HashMap::new(),
            hisense: HashMap::new(),
            states: HashMap::new(),
        }
    }
    pub async fn execute(&mut self, id: String, command: Command) -> Result<State, String> {
        let result = tokio::time::timeout(Duration::from_secs(55), self.inner(&id, command)).await;
        match result {
            Ok(r) => r,
            Err(_) => {
                self.samsung.remove(&id);
                self.hisense.remove(&id);
                Err("The remote timed out. Check its network connection and pairing.".into())
            }
        }
    }
    async fn inner(&mut self, id: &str, command: Command) -> Result<State, String> {
        let brand = command.brand;
        let mut c = self.store.get(id, brand)?;
        let state_key = format!("{id}/{brand:?}");
        let mut state = self.states.get(&state_key).cloned().unwrap_or_default();
        state.note = self.store.warning.clone();
        match command.action {
            Action::Info => {}
            Action::Connect => {
                if !c.public(brand).paired {
                    return Err("Pair this network remote first.".into());
                }
                if brand == Brand::Samsung {
                    let client = samsung::Client::connect(&mut c).await?;
                    self.store.save(id, brand, c)?;
                    self.samsung.insert(id.into(), client);
                } else {
                    self.ensure_hisense(id, &mut c).await?;
                }
                state.note = Some("Remote connection established.".into());
            }
            Action::Discover => {
                state.devices = network::discover(brand).await?;
                state.note = Some(
                    if state.devices.is_empty() {
                        "No device found. You can enter its IPv4 address manually."
                    } else {
                        "Select the device that belongs to this screen."
                    }
                    .into(),
                );
            }
            Action::Save {
                ip_address,
                mac_address,
                certificate_path,
                device_uuid,
                controller_mac,
            } => {
                let ip = network::address(&ip_address)?.to_string();
                let mac = if mac_address.trim().is_empty() {
                    String::new()
                } else {
                    network::mac(&mac_address)?
                };
                if c.ip_address != ip {
                    c.token.clear();
                    c.client_id.clear();
                    c.access_token.clear();
                    c.refresh_token.clear();
                    c.legacy_authorized = false;
                    c.controller_mac_address.clear();
                    c.protocol_version = None;
                    c.certificate_fingerprint.clear();
                }
                c.ip_address = ip;
                c.mac_address = mac;
                c.device_uuid = device_uuid;
                if brand == Brand::Hisense {
                    c.client_certificate_path = self.certificate(&certificate_path)?;
                    c.controller_mac_address = network::chosen_controller(
                        network::controller_mac(&c.ip_address).await,
                        &controller_mac,
                    )?;
                }
                self.store.save(id, brand, c)?;
                self.samsung.remove(id);
                self.hisense.remove(id);
                state.note = Some("Address saved for this monitor.".into());
            }
            Action::Pair {
                ip_address,
                mac_address,
                pin,
            } if brand == Brand::Samsung => {
                // A new pairing on its own copy: without the token the TV may have revoked,
                // and the saved association stays until this one is accepted.
                let mut paired = c.clone();
                paired.token.clear();
                paired.certificate_fingerprint.clear();
                paired.ip_address = network::address(&ip_address)?.to_string();
                let (device, _, _) = network::probe(&paired.ip_address, brand).await?;
                paired.name = device.name;
                paired.mac_address = if device.mac_address.is_empty() {
                    mac_address
                } else {
                    device.mac_address
                };
                let client = samsung::Client::connect(&mut paired).await?;
                self.store.save(id, brand, paired)?;
                self.samsung.insert(id.into(), client);
                state.note = Some("Samsung paired with this monitor.".into());
                let _ = pin;
            }
            Action::RequestPin | Action::Pair { .. } => {
                if brand != Brand::Hisense {
                    return Err("This remote does not use a PIN.".into());
                }
                network::address(&c.ip_address)?;
                // The PIN belongs to the display the form shows, which must be the saved one.
                if let Action::Pair { ip_address, .. } = &command.action {
                    if network::address(ip_address)?.to_string() != c.ip_address {
                        return Err("Save this address before pairing it.".into());
                    }
                }
                c.client_certificate_path = self.certificate(&c.client_certificate_path)?;
                let request = matches!(command.action, Action::RequestPin);
                if request || !self.hisense.get(id).is_some_and(|client| client.alive()) {
                    if let Ok((d, protocol, brand)) =
                        network::probe(&c.ip_address, Brand::Hisense).await
                    {
                        c.name = d.name;
                        c.protocol_version = protocol;
                        c.brand = brand;
                        if !d.mac_address.is_empty() {
                            c.mac_address = d.mac_address;
                        }
                    } else if c.protocol_version.is_none() && id.starts_with("HEC002F") {
                        c.protocol_version = Some(2160);
                        c.brand = "his".into();
                    }
                    let client = hisense::begin(&mut c, request).await?;
                    self.hisense.insert(id.into(), client);
                }
                if let Action::Pair { pin, .. } = command.action {
                    hisense::pair(self.hisense.get(id).unwrap(), &mut c, &pin).await?;
                    state.note = Some("VIDAA paired with this monitor.".into());
                } else {
                    state.note =
                        Some("Enter the four-digit VIDAA PIN displayed on the screen.".into());
                }
                self.store.save(id, brand, c)?;
            }
            Action::Wake => {
                network::wake(&c.mac_address).await?;
                state.note = Some("Wake-on-LAN sent.".into());
            }
            Action::Macro { sequence } => {
                let steps = sequence_steps(&sequence)?;
                for (key, delay) in steps {
                    self.key(id, brand, &mut c, key).await?;
                    tokio::time::sleep(delay).await;
                }
                if brand == Brand::Samsung {
                    c.picture_macro = sequence;
                } else {
                    c.key_macro = sequence;
                }
                self.store.save(id, brand, c)?;
                state.note = Some("Macro sent.".into());
            }
            Action::Key { key } => {
                validate_key(&key)?;
                self.key(id, brand, &mut c, key).await?;
                self.store.save(id, brand, c)?;
            }
            action => {
                if brand != Brand::Hisense {
                    return Err("This command requires a VIDAA device.".into());
                }
                self.ensure_hisense(id, &mut c).await?;
                hisense::command(self.hisense.get(id).unwrap(), &c, action, &mut state).await?;
            }
        }
        state.samsung = self.store.get(id, Brand::Samsung)?.public(Brand::Samsung);
        state.hisense = self.store.get(id, Brand::Hisense)?.public(Brand::Hisense);
        self.states.insert(state_key, state.clone());
        Ok(state)
    }
    async fn ensure_hisense(&mut self, id: &str, c: &mut Saved) -> Result<(), String> {
        if !self.hisense.get(id).is_some_and(|client| client.alive()) {
            let client = hisense::reopen(c).await?;
            // An association older than this check learns its device here.
            if c.certificate_fingerprint != client.fingerprint {
                c.certificate_fingerprint = client.fingerprint.clone();
                self.store.save(id, Brand::Hisense, c.clone())?;
            }
            self.hisense.insert(id.into(), client);
        }
        Ok(())
    }
    async fn key(
        &mut self,
        id: &str,
        brand: Brand,
        c: &mut Saved,
        key: String,
    ) -> Result<(), String> {
        validate_key(&key)?;
        if brand == Brand::Samsung {
            if !self
                .samsung
                .get(id)
                .is_some_and(|client| client.alive() && client.ip == c.ip_address)
            {
                let client = samsung::Client::connect(c).await?;
                self.samsung.insert(id.into(), client);
            }
            self.samsung.get(id).unwrap().key(key).await
        } else {
            self.ensure_hisense(id, c).await?;
            self.hisense
                .get(id)
                .unwrap()
                .publish(
                    &hisense::topic(c, "remote_service", "sendkey"),
                    hisense::key(&key),
                )
                .await
        }
    }
    fn certificate(&self, path: &str) -> Result<String, String> {
        if !path.trim().is_empty() {
            let path = if let Some(s) = path.strip_prefix("~/") {
                PathBuf::from(
                    std::env::var_os("HOME").ok_or("Cannot expand the certificate path.")?,
                )
                .join(s)
            } else {
                PathBuf::from(path)
            };
            if !path.is_file() {
                return Err("The VIDAA client certificate file does not exist.".into());
            }
            return Ok(path.to_string_lossy().into());
        }
        for name in ["vidaa-client.p12", "client_mobile_android.p12", "3R.p12"] {
            let p = self.store.directory().join(name);
            if p.is_file() {
                return Ok(p.to_string_lossy().into());
            }
        }
        Ok(String::new())
    }
}
fn validate_key(key: &str) -> Result<(), String> {
    if !key.starts_with("KEY_")
        || key.len() > 64
        || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        Err("Invalid remote key.".into())
    } else {
        Ok(())
    }
}
fn sequence_steps(sequence: &str) -> Result<Vec<(String, Duration)>, String> {
    let mut steps: Vec<(String, Duration)> = vec![];
    for part in sequence
        .split([',', ';', '+', '\n', '\r'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        if let Ok(delay) = part.parse::<u64>() {
            if delay > 10000 {
                return Err("Macro delays must not exceed 10 seconds.".into());
            }
            steps.last_mut().ok_or("A delay must follow a key.")?.1 = Duration::from_millis(delay);
        } else {
            let key = part.to_uppercase();
            validate_key(&key)?;
            steps.push((key, Duration::from_millis(150)));
        }
    }
    if steps.is_empty()
        || steps.len() > 50
        || steps.iter().map(|s| s.1).sum::<Duration>() > Duration::from_secs(15)
    {
        return Err("Use 1–50 remote keys and at most 15 seconds of delays.".into());
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn service_with(brand: Brand, saved: Saved) -> (tempfile::TempDir, Service) {
        let dir = tempfile::tempdir().unwrap();
        let mut service = Service::new(dir.path().into());
        service.store.save("screen", brand, saved).unwrap();
        (dir, service)
    }
    #[tokio::test]
    async fn a_vidaa_pin_only_goes_to_the_saved_display() {
        let (_dir, mut service) = service_with(
            Brand::Hisense,
            Saved {
                ip_address: "192.0.2.1".into(),
                ..Default::default()
            },
        );
        let command = Command {
            brand: Brand::Hisense,
            action: Action::Pair {
                ip_address: "192.0.2.2".into(),
                mac_address: String::new(),
                pin: "1234".into(),
            },
        };
        let error = service.inner("screen", command).await.err().unwrap();
        assert!(error.contains("Save this address"), "{error}");
    }
    #[tokio::test]
    async fn a_failed_samsung_pairing_keeps_the_saved_association() {
        let saved = Saved {
            ip_address: "192.0.2.1".into(),
            token: "still-valid".into(),
            certificate_fingerprint: "known".into(),
            ..Default::default()
        };
        let (_dir, mut service) = service_with(Brand::Samsung, saved);
        let command = Command {
            brand: Brand::Samsung,
            action: Action::Pair {
                // Nothing listens on the loopback's Samsung port.
                ip_address: "127.0.0.1".into(),
                mac_address: String::new(),
                pin: String::new(),
            },
        };
        assert!(service.inner("screen", command).await.is_err());
        let kept = service.store.get("screen", Brand::Samsung).unwrap();
        assert_eq!(kept.ip_address, "192.0.2.1");
        assert_eq!(kept.token, "still-valid");
        assert_eq!(kept.certificate_fingerprint, "known");
    }
    #[test]
    fn macros_are_bounded_and_preserve_per_key_delays() {
        let steps = sequence_steps("KEY_MENU,700,KEY_DOWN,KEY_ENTER").unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].1, Duration::from_millis(700));
        for invalid in [
            "",
            "700,KEY_UP",
            "KEY_UP,10001",
            "KEY UP",
            "KEY_UP,10000,KEY_DOWN,10000",
        ] {
            assert!(sequence_steps(invalid).is_err(), "{invalid}");
        }
    }
}
