use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use lbm_ipc::remote::{Brand, Config};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
};

// Compatible with the old encrypted files, but Rust writes its own file so that
// a running older frontend cannot overwrite a newly paired Rust session.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct Saved {
    pub ip_address: String,
    pub mac_address: String,
    pub name: String,
    pub token: String,
    pub picture_macro: String,
    pub key_macro: String,
    pub controller_mac_address: String,
    pub device_uuid: String,
    pub client_certificate_path: String,
    pub client_certificate_password: String,
    pub protocol_version: Option<u32>,
    pub brand: String,
    pub client_id: String,
    pub mqtt_username: String,
    pub access_token: String,
    pub refresh_token: String,
    pub legacy_authorized: bool,
}
impl Saved {
    pub fn public(&self, brand: Brand) -> Config {
        Config {
            ip_address: self.ip_address.clone(),
            mac_address: self.mac_address.clone(),
            name: if self.name.is_empty() {
                brand.label().into()
            } else {
                self.name.clone()
            },
            paired: match brand {
                Brand::Samsung => !self.token.is_empty(),
                Brand::Hisense => self.legacy_authorized || !self.access_token.is_empty(),
            },
            certificate_path: self.client_certificate_path.clone(),
            device_uuid: self.device_uuid.clone(),
            controller_mac: self.controller_mac_address.clone(),
            r#macro: if brand == Brand::Samsung {
                self.picture_macro.clone()
            } else {
                self.key_macro.clone()
            },
        }
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct Data {
    samsung: BTreeMap<String, Saved>,
    hisense: BTreeMap<String, Saved>,
}
pub struct Store {
    dir: PathBuf,
    data: Option<Data>,
    pub warning: Option<String>,
}
impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            data: None,
            warning: None,
        }
    }
    pub fn directory(&self) -> &Path {
        &self.dir
    }
    fn load(&mut self) -> Result<&mut Data, String> {
        if self.data.is_none() {
            let file = self.dir.join("network-remotes.json");
            let mut data: Data = if file.exists() {
                serde_json::from_slice(&self.read(&file)?).map_err(|e| e.to_string())?
            } else {
                Data::default()
            };
            for (name, target) in [
                ("samsung-tizen.json", &mut data.samsung),
                ("hisense-vidaa.json", &mut data.hisense),
            ] {
                let file = self.dir.join(name);
                if file.exists() {
                    match self.read(&file).and_then(|bytes|serde_json::from_slice::<BTreeMap<String,Saved>>(&bytes).map_err(|e|e.to_string())) {
                        Ok(original) => for (id,config) in original { target.entry(id).or_insert(config); },
                        Err(_) => self.warning = Some(format!("The old {name} association could not be imported. You can pair this remote again here; the original file is preserved.")),
                    }
                }
            }
            self.data = Some(data);
        }
        Ok(self.data.as_mut().unwrap())
    }
    pub fn get(&mut self, id: &str, brand: Brand) -> Result<Saved, String> {
        let d = self.load()?;
        Ok(match brand {
            Brand::Samsung => &d.samsung,
            Brand::Hisense => &d.hisense,
        }
        .get(id)
        .cloned()
        .unwrap_or_default())
    }
    pub fn save(&mut self, id: &str, brand: Brand, config: Saved) -> Result<(), String> {
        // Commit memory only after the encrypted file has reached disk.
        let mut data = self.load()?.clone();
        match brand {
            Brand::Samsung => &mut data.samsung,
            Brand::Hisense => &mut data.hisense,
        }
        .insert(id.into(), config);
        let bytes = serde_json::to_vec(&data).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let key = self.key(true)?;
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
        let encrypted = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "Invalid remote encryption key")?
            .encrypt(Nonce::from_slice(&nonce), bytes.as_ref())
            .map_err(|_| "Cannot encrypt remote settings")?;
        // C# stores nonce | tag | ciphertext; aes-gcm returns ciphertext | tag.
        let split = encrypted.len() - 16;
        let envelope = format!(
            "LBM1.aesgcm.{}",
            STANDARD.encode([&nonce[..], &encrypted[split..], &encrypted[..split]].concat())
        );
        let path = self.dir.join("network-remotes.json");
        let temporary = self
            .dir
            .join(format!("network-remotes.{}.tmp", std::process::id()));
        let mut file = private_options()
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        let result = (|| {
            file.write_all(envelope.as_bytes())?;
            file.sync_all()?;
            std::fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|e| e.to_string())?;
        self.data = Some(data);
        Ok(())
    }
    fn read(&self, file: &Path) -> Result<Vec<u8>, String> {
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        if !text.starts_with("LBM1.") {
            return Ok(text.into_bytes());
        }
        let encoded = text.strip_prefix("LBM1.aesgcm.").ok_or("This pairing file uses an encryption format unavailable here. Pair this display again.")?;
        let bytes = STANDARD
            .decode(encoded.trim())
            .map_err(|_| "Invalid encrypted remote settings")?;
        if bytes.len() < 28 {
            return Err("Truncated encrypted remote settings".into());
        }
        let key = self.key(false)?;
        let payload = [&bytes[28..], &bytes[12..28]].concat();
        Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "Invalid encryption key")?
            .decrypt(Nonce::from_slice(&bytes[..12]), payload.as_ref())
            .map_err(|_| "Cannot decrypt the saved remote association; check secrets.key.".into())
    }
    fn key(&self, create: bool) -> Result<Vec<u8>, String> {
        let path = self.dir.join("secrets.key");
        if create && !path.exists() {
            let mut key = [0; 32];
            getrandom::fill(&mut key).map_err(|e| e.to_string())?;
            match private_options().create_new(true).open(&path) {
                Ok(mut file) => file
                    .write_all(STANDARD.encode(key).as_bytes())
                    .map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        let key = STANDARD
            .decode(
                std::fs::read_to_string(path)
                    .map_err(|e| e.to_string())?
                    .trim(),
            )
            .map_err(|_| "Unreadable remote encryption key")?;
        if key.len() != 32 {
            return Err("Invalid remote encryption key length".into());
        }
        Ok(key)
    }
}
fn private_options() -> std::fs::OpenOptions {
    let mut o = std::fs::OpenOptions::new();
    o.write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn associations_are_encrypted_imported_and_never_exposed_in_public_state() {
        let dir = tempfile::tempdir().unwrap();
        let original =
            r#"{"SAM1":{"IpAddress":"192.168.1.20","Token":"original-token","Name":"Samsung"}}"#;
        std::fs::write(dir.path().join("samsung-tizen.json"), original).unwrap();
        let mut store = Store::new(dir.path().into());
        let config = store.get("SAM1", Brand::Samsung).unwrap();
        assert_eq!(config.token, "original-token");
        assert!(!serde_json::to_string(&config.public(Brand::Samsung))
            .unwrap()
            .contains("original-token"));
        store.save("SAM1", Brand::Samsung, config).unwrap();
        let encrypted = std::fs::read_to_string(dir.path().join("network-remotes.json")).unwrap();
        assert!(encrypted.starts_with("LBM1.aesgcm."));
        assert!(!encrypted.contains("original-token"));
        let mut reopened = Store::new(dir.path().into());
        assert_eq!(
            reopened.get("SAM1", Brand::Samsung).unwrap().token,
            "original-token"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("samsung-tizen.json")).unwrap(),
            original
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.path().join("secrets.key"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn unreadable_legacy_file_can_be_repaired_without_overwriting_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hisense-vidaa.json");
        std::fs::write(&path, "LBM1.dpapi.unavailable").unwrap();
        let mut store = Store::new(dir.path().into());
        assert!(store
            .get("HEC1", Brand::Hisense)
            .unwrap()
            .access_token
            .is_empty());
        assert!(store.warning.is_some());
        store
            .save("HEC1", Brand::Hisense, Saved::default())
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "LBM1.dpapi.unavailable"
        );
    }
}
