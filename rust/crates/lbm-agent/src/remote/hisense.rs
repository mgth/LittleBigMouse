use super::{
    mqtt::{self, Client},
    network,
    store::Saved,
};
use lbm_ipc::remote::{Action, Brand, PictureSetting, State};
use md5::{Digest, Md5};
use serde_json::{json, Value};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn legacy(c: &Saved) -> bool {
    c.protocol_version.is_some_and(|v| v > 0 && v < 3000)
}
fn topics(id: &str, is_legacy: bool) -> Vec<String> {
    if is_legacy {
        return vec![
            format!("/remoteapp/mobile/{id}/#"),
            "/remoteapp/mobile/broadcast/#".into(),
        ];
    }
    [
        "ui_service/data/authentication",
        "ui_service/data/authenticationcode",
        "ui_service/data/authenticationcodetoast",
        "ui_service/data/authenticationcodeclose",
        "ui_service/data/tokenissuance",
        "platform_service/data/tokenissuance",
    ]
    .into_iter()
    .map(|suffix| format!("/remoteapp/mobile/{id}/{suffix}"))
    .chain(
        [
            "/remoteapp/mobile/broadcast/ui_service/state",
            "/remoteapp/mobile/broadcast/platform_service/actions/volumechange",
            "/remoteapp/mobile/broadcast/platform_service/data/picturesetting",
        ]
        .into_iter()
        .map(str::to_owned),
    )
    .collect()
}

pub fn topic(c: &Saved, service: &str, action: &str) -> String {
    format!("/remoteapp/tv/{service}/{}/actions/{action}", c.client_id)
}
fn session_id() -> String {
    let mut b = [0; 8];
    let _ = getrandom::fill(&mut b);
    format!("LittleBigMouse/{:016x}", u64::from_ne_bytes(b))
}
pub async fn reopen(c: &Saved) -> Result<Client, String> {
    if !c.public(Brand::Hisense).paired {
        return Err("Pair the VIDAA display first.".into());
    }
    let (id, user, pass) = if legacy(c) {
        (
            session_id(),
            "hisenseservice".into(),
            "multimqttservice".into(),
        )
    } else {
        (
            c.client_id.clone(),
            c.mqtt_username.clone(),
            c.access_token.clone(),
        )
    };
    Client::connect(c, &id, &user, &pass, &topics(&c.client_id, legacy(c))).await
}
/// Asking for a PIN is where the user says that this device is the right one, so it
/// starts over and nothing stored goes out. Entering the PIN continues that exchange:
/// after a lost connection it must still be the device that showed it.
fn prepare_pairing(c: &mut Saved, request_pin: bool) {
    if request_pin {
        c.certificate_fingerprint.clear();
        c.access_token.clear();
        c.refresh_token.clear();
    }
}
pub async fn begin(c: &mut Saved, request_pin: bool) -> Result<Client, String> {
    prepare_pairing(c, request_pin);
    if c.controller_mac_address.is_empty() {
        c.controller_mac_address = network::controller_mac(&c.ip_address).await?;
    }
    let client = if legacy(c) {
        let identity = network::mac(&c.controller_mac_address)?;
        c.device_uuid = identity.clone();
        c.client_id = format!("{identity}$normal");
        c.mqtt_username = "hisenseservice".into();
        c.access_token.clear();
        c.refresh_token.clear();
        c.legacy_authorized = false;
        Client::connect(
            c,
            &session_id(),
            "hisenseservice",
            "multimqttservice",
            &topics(&c.client_id, legacy(c)),
        )
        .await?
    } else {
        let identity = network::mac(if c.device_uuid.is_empty() {
            &c.controller_mac_address
        } else {
            &c.device_uuid
        })?;
        let variants = [identity.clone(), identity.to_lowercase()];
        let methods = match c.protocol_version {
            Some(v) if v >= 3290 => vec![2],
            Some(_) => vec![1],
            None => vec![2, 1, 0],
        };
        let mut found = None;
        for method in methods {
            for identity in &variants {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_secs();
                let brand = if c.brand.is_empty() || c.brand.to_lowercase().contains("hisense") {
                    "his"
                } else {
                    c.brand.as_str()
                };
                let (id, user, pass) = credentials(identity, method, timestamp, brand);
                if let Ok(Ok(client)) = tokio::time::timeout(
                    Duration::from_secs(6),
                    Client::connect(c, &id, &user, &pass, &topics(&id, false)),
                )
                .await
                {
                    c.client_id = id;
                    c.mqtt_username = user;
                    c.device_uuid = identity.clone();
                    c.access_token.clear();
                    c.refresh_token.clear();
                    found = Some(client);
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.ok_or("VIDAA rejected the authentication variants. Check its date, time and pairing settings.")?
    };
    c.certificate_fingerprint = client.fingerprint.clone();
    if request_pin {
        let action = if legacy(c) {
            "gettvstate"
        } else {
            "vidaa_app_connect"
        };
        let payload = if legacy(c) {
            String::new()
        } else {
            json!({"app_version":2,"connect_result":0,"device_type":"Mobile App"}).to_string()
        };
        client
            .publish(&topic(c, "ui_service", action), &payload)
            .await?;
    }
    Ok(client)
}
pub fn credentials(
    identity: &str,
    method: u8,
    timestamp: u64,
    brand: &str,
) -> (String, String, String) {
    let hash = |s: &str| format!("{:X}", Md5::digest(s.as_bytes()));
    let race = hash(&format!("38D65DC30F45109A369A86FCE866A85B${identity}"));
    let id = format!("{identity}${brand}${}_vidaacommon_001", &race[..6]);
    let time = if method == 0 {
        timestamp
    } else {
        timestamp ^ 0x5698_1477_2b03_a968
    };
    let user = format!("{brand}${time}");
    let remainder = timestamp
        .to_string()
        .bytes()
        .map(|b| u32::from(b - b'0'))
        .sum::<u32>()
        % 10;
    let suffix = if method == 2 {
        "h!i@s#$v%i^d&a*a"
    } else {
        "h*i&s%e!r^v0i1c9"
    };
    let value = hash(&format!("{brand}{remainder}{suffix}"));
    let pass = hash(&format!("{timestamp}${}", &value[..6]));
    (id, user, pass)
}
pub async fn pair(client: &Client, c: &mut Saved, pin: &str) -> Result<(), String> {
    if pin.len() != 4 || !pin.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Enter the four-digit VIDAA PIN.".into());
    }
    let mut rx = client.subscribe_events();
    let payload = if legacy(c) {
        json!({"authNum":pin})
    } else {
        json!({"authNum":pin.parse::<u32>().unwrap()})
    };
    client
        .publish(
            &topic(c, "ui_service", "authenticationcode"),
            &payload.to_string(),
        )
        .await?;
    mqtt::answer(&mut rx, |e| {
        if e.topic.contains("tokenissuance") {
            if let Ok(v) = serde_json::from_str::<Value>(&e.payload) {
                issued(c, &v);
            }
        }
        if !e.topic.contains("authentication") {
            return None;
        }
        if e.payload.is_empty() {
            return Some(Ok(()));
        }
        let value: Value = serde_json::from_str(&e.payload).ok()?;
        value["result"].as_i64().map(|r| {
            if r == 1 {
                Ok(())
            } else {
                Err("The VIDAA PIN was rejected.".into())
            }
        })
    })
    .await?;
    if legacy(c) {
        c.legacy_authorized = true;
        return Ok(());
    }
    client
        .publish(
            &format!(
                "/remoteapp/tv/platform_service/{}/data/gettoken",
                c.client_id
            ),
            "{\"refreshtoken\":\"\"}",
        )
        .await?;
    client
        .publish(&topic(c, "ui_service", "authenticationcodeclose"), "")
        .await?;
    if c.access_token.is_empty() {
        mqtt::answer(&mut rx, |e| {
            if !e.topic.contains("tokenissuance") {
                return None;
            }
            let v = serde_json::from_str::<Value>(&e.payload).ok()?;
            issued(c, &v).then_some(Ok(()))
        })
        .await?;
    }
    Ok(())
}
fn issued(c: &mut Saved, v: &Value) -> bool {
    if let Some(token) = v["accesstoken"].as_str().filter(|s| !s.is_empty()) {
        c.access_token = token.into();
        c.refresh_token = v["refreshtoken"].as_str().unwrap_or("").into();
        true
    } else {
        false
    }
}
pub fn key(key: &str) -> &str {
    match key {
        "KEY_ENTER" => "KEY_OK",
        "KEY_RETURN" => "KEY_RETURNS",
        "KEY_VOLUP" => "KEY_VOLUMEUP",
        "KEY_VOLDOWN" => "KEY_VOLUMEDOWN",
        "KEY_CHUP" => "KEY_CHANNELUP",
        "KEY_CHDOWN" => "KEY_CHANNELDOWN",
        "KEY_PLAYPAUSE" => "KEY_PLAY",
        "KEY_FF" => "KEY_FORWARDS",
        "KEY_REWIND" => "KEY_BACKWARDS",
        _ => key,
    }
}
pub async fn command(
    client: &Client,
    c: &Saved,
    action: Action,
    state: &mut State,
) -> Result<(), String> {
    let mut rx = client.subscribe_events();
    match action {
        Action::Key { key: k } => {
            client
                .publish(&topic(c, "remote_service", "sendkey"), key(&k))
                .await?
        }
        Action::VolumeRead | Action::VolumeSet { .. } => {
            let (name, payload) = match action {
                Action::VolumeSet { value } => {
                    if !(0..=100).contains(&value) {
                        return Err("Volume must be between 0 and 100.".into());
                    }
                    ("changevolume", value.to_string())
                }
                _ => ("getvolume", "0".into()),
            };
            client
                .publish(&topic(c, "platform_service", name), &payload)
                .await?;
            state.volume = Some(
                mqtt::answer(&mut rx, |e| {
                    if !e.topic.ends_with("/platform_service/actions/volumechange") {
                        return None;
                    }
                    let v: Value = serde_json::from_str(&e.payload).ok()?;
                    v["volume_value"]
                        .as_i64()
                        .filter(|v| (0..=100).contains(v))
                        .map(|v| Ok(v as i32))
                })
                .await?,
            );
        }
        Action::PictureRead | Action::PictureSet { .. } => {
            if let Action::PictureSet { menu_id, value } = action {
                if !(1..=999).contains(&menu_id) || !(-1000..=1000).contains(&value) {
                    return Err("Invalid VIDAA picture-setting value.".into());
                }
                client.publish(&topic(c,"platform_service","picturesetting"),&json!({"action":"set_value","menu_id":menu_id,"menu_value":value,"menu_value_type":"int"}).to_string()).await?;
            }
            client
                .publish(
                    &topic(c, "platform_service", "picturesetting"),
                    "{\"action\":\"get_menu_info\"}",
                )
                .await?;
            state.picture_settings = mqtt::answer(&mut rx, |e| {
                if !e.topic.ends_with("/platform_service/data/picturesetting") {
                    return None;
                }
                Some(pictures(&e.payload))
            })
            .await?;
        }
        Action::Platform { name, value } => {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !(0..=10).contains(&value)
            {
                return Err("Use a valid platform action and a level between 0 and 10.".into());
            }
            client
                .publish(&topic(c, "platform_service", &name), &value.to_string())
                .await?;
            state.note = Some("Command sent; this experimental setting has no readback.".into());
        }
        _ => return Err("Unsupported VIDAA command.".into()),
    }
    Ok(())
}
fn pictures(text: &str) -> Result<Vec<PictureSetting>, String> {
    let root: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let values = root.get("menu_info").unwrap_or(&root);
    let items = values
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![values.clone()]);
    Ok(items
        .iter()
        .filter_map(|v| {
            Some(PictureSetting {
                menu_id: v["menu_id"].as_i64()? as i32,
                name: v["menu_name"].as_str().unwrap_or("").into(),
                value: v["menu_value"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v["menu_value"].to_string()),
                value_type: v["menu_value_type"].as_str().unwrap_or("").into(),
                flag: v["menu_flag"].as_i64().map(|i| i as i32),
            })
        })
        .collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_matches_the_reference_known_vector() {
        let (id, user, pass) = credentials("56:b8:88:4e:f7:19", 2, 1766974704, "his");
        assert_eq!(id, "56:b8:88:4e:f7:19$his$256DBF_vidaacommon_001");
        assert_eq!(user, "his$6239759786168176024");
        assert_eq!(pass, "C3BA44782E18ABF4892AC44D79A622D2");
        assert_eq!(key("KEY_ENTER"), "KEY_OK");
        assert_eq!(key("KEY_VOLDOWN"), "KEY_VOLUMEDOWN");
    }
    #[test]
    fn real_picture_catalog_keeps_menu_ids_and_values() {
        let settings=pictures(r#"{"menu_info":[{"menu_id":2,"menu_name":"Laser Luminance","menu_value":5,"menu_value_type":"int","menu_flag":1}]}"#).unwrap();
        assert_eq!(settings[0].menu_id, 2);
        assert_eq!(settings[0].value, "5");
    }
    #[test]
    fn a_pin_is_only_entered_on_the_device_that_showed_it() {
        let saved = Saved {
            certificate_fingerprint: "shown".into(),
            access_token: "old".into(),
            refresh_token: "old".into(),
            ..Default::default()
        };
        let mut resumed = saved.clone();
        prepare_pairing(&mut resumed, false);
        assert_eq!(resumed.certificate_fingerprint, "shown");
        assert!(network::trusted(&resumed, "shown").is_ok());
        assert!(network::trusted(&resumed, "another").is_err());
        let mut restarted = saved;
        prepare_pairing(&mut restarted, true);
        assert!(restarted.certificate_fingerprint.is_empty());
        assert!(network::trusted(&restarted, "another").is_ok());
    }
}
