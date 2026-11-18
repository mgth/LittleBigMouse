use super::store::Saved;
use lbm_ipc::remote::{Brand, Device};
use std::{collections::BTreeSet, net::Ipv4Addr, time::Duration};
use tokio::net::{TcpStream, UdpSocket};

pub fn address(s: &str) -> Result<Ipv4Addr, String> {
    let ip: Ipv4Addr = s
        .trim()
        .parse()
        .map_err(|_| "Enter the device's IPv4 address.")?;
    if ip.is_unspecified() || ip.is_multicast() || ip.is_broadcast() {
        return Err("Enter a unicast device address.".into());
    }
    Ok(ip)
}
pub fn mac(value: &str) -> Result<String, String> {
    let compact: String = value.chars().filter(|c| *c != ':' && *c != '-').collect();
    if compact.len() != 12 || !compact.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Enter a MAC address with six hexadecimal pairs.".into());
    }
    Ok((0..6)
        .map(|i| compact[i * 2..i * 2 + 2].to_uppercase())
        .collect::<Vec<_>>()
        .join(":"))
}
pub async fn tls(
    c: &Saved,
    port: u16,
    hisense: bool,
) -> Result<tokio_native_tls::TlsStream<TcpStream>, String> {
    let ip = address(&c.ip_address)?;
    let mut builder = native_tls::TlsConnector::builder();
    // Only an explicit device IPv4 endpoint uses its self-signed certificate.
    builder
        .danger_accept_invalid_certs(true)
        .danger_accept_invalid_hostnames(true);
    if hisense {
        builder
            .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
            .max_protocol_version(Some(native_tls::Protocol::Tlsv12));
        if !c.client_certificate_path.is_empty() {
            let file = std::fs::read(&c.client_certificate_path)
                .map_err(|e| format!("VIDAA certificate: {e}"))?;
            let password = if c.client_certificate_password.is_empty() {
                "186e990688070325a1c4b0ce275d2388"
            } else {
                &c.client_certificate_password
            };
            builder.identity(
                native_tls::Identity::from_pkcs12(&file, password)
                    .map_err(|e| format!("Cannot load VIDAA certificate: {e}"))?,
            );
        }
    }
    let connector =
        tokio_native_tls::TlsConnector::from(builder.build().map_err(|e| e.to_string())?);
    let socket = tokio::time::timeout(Duration::from_secs(6), TcpStream::connect((ip, port)))
        .await
        .map_err(|_| {
            format!(
                "No response from {ip}:{port}. Check the device address and network connection."
            )
        })?
        .map_err(|e| format!("Cannot reach {ip}:{port}: {e}"))?;
    tokio::time::timeout(
        Duration::from_secs(10),
        connector.connect(&ip.to_string(), socket),
    )
    .await
    .map_err(|_| "The remote TLS handshake timed out.".to_string())?
    .map_err(|e| format!("Remote TLS connection: {e}"))
}
async fn http(ip: Ipv4Addr, port: u16, path: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(format!("http://{ip}:{port}{path}"))
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    if response.content_length().is_some_and(|n| n > 1024 * 1024) {
        return Err("Device description is too large.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 1024 * 1024 {
            return Err("Device description is too large.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(text)
}
pub async fn probe(ip: &str, brand: Brand) -> Result<(Device, Option<u32>, String), String> {
    let ip = address(ip)?;
    if brand == Brand::Samsung {
        let text = http(ip, 8001, "/api/v2/").await?;
        let root: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let d = root.get("device").unwrap_or(&root);
        let string = |keys: &[&str]| {
            keys.iter()
                .find_map(|k| d[*k].as_str())
                .unwrap_or("")
                .to_owned()
        };
        return Ok((
            Device {
                ip_address: ip.to_string(),
                name: string(&["name"]),
                model_name: string(&["modelName", "model"]),
                mac_address: string(&["wifiMac", "networkMac", "mac"]),
            },
            None,
            "".into(),
        ));
    }
    for port in [18400, 38400] {
        if let Ok(text) = http(ip, port, "/MediaServer/rendererdevicedesc.xml").await {
            let doc = roxmltree::Document::parse(&text).map_err(|e| e.to_string())?;
            let field = |key| {
                doc.descendants()
                    .find(|n| n.has_tag_name(key))
                    .and_then(|n| n.text())
                    .unwrap_or("")
                    .trim()
                    .to_string()
            };
            let desc = field("modelDescription");
            let data: std::collections::HashMap<_, _> = desc
                .split(['\n', '\r', ';'])
                .filter_map(|s| s.split_once('='))
                .map(|(k, v)| (k.trim().to_lowercase(), v.trim()))
                .collect();
            let get = |key: &str| data.get(key).copied().unwrap_or("");
            if get("vidaa_support") != "1" && get("transport_protocol").is_empty() {
                continue;
            }
            let hardware_mac = ["mac", "macwifi", "wifi_mac", "macaddress"]
                .into_iter()
                .map(get)
                .find(|s| !s.is_empty())
                .unwrap_or("");
            return Ok((
                Device {
                    ip_address: ip.to_string(),
                    name: field("friendlyName"),
                    model_name: field("modelName"),
                    mac_address: mac(hardware_mac).unwrap_or_default(),
                },
                get("transport_protocol").parse().ok(),
                get("brand").to_string(),
            ));
        }
    }
    Err(
        "The VIDAA device description is unavailable. Check the IP address and network connection."
            .into(),
    )
}
pub async fn discover(brand: Brand) -> Result<Vec<Device>, String> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|e| e.to_string())?;
    socket.send_to(b"M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 2\r\nST: ssdp:all\r\n\r\n", (Ipv4Addr::new(239,255,255,250),1900)).await.map_err(|e| e.to_string())?;
    let mut addresses = BTreeSet::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let mut buffer = [0; 8192];
    while let Ok(Ok((_, sender))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buffer)).await
    {
        if let std::net::IpAddr::V4(ip) = sender.ip() {
            addresses.insert(ip);
        }
        if addresses.len() >= 64 {
            break;
        }
    }
    use futures_util::{stream, StreamExt};
    Ok(stream::iter(addresses)
        .map(|ip| async move { probe(&ip.to_string(), brand).await.ok().map(|r| r.0) })
        .buffer_unordered(12)
        .filter_map(|v| async { v })
        .collect()
        .await)
}
pub async fn wake(mac_address: &str) -> Result<(), String> {
    let normalized = mac(mac_address)?;
    let bytes: Vec<_> = normalized
        .split(':')
        .map(|s| u8::from_str_radix(s, 16).unwrap())
        .collect();
    let mut packet = vec![255; 6];
    for _ in 0..16 {
        packet.extend(&bytes);
    }
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .await
        .map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    for _ in 0..3 {
        socket
            .send_to(&packet, (Ipv4Addr::BROADCAST, 9))
            .await
            .map_err(|e| e.to_string())?;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    Ok(())
}
#[cfg(target_os = "linux")]
pub async fn controller_mac(ip: &str) -> Result<String, String> {
    let ip = address(ip)?;
    let output = tokio::process::Command::new("ip")
        .args(["-j", "route", "get", &ip.to_string()])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    let routes: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    let dev = routes[0]["dev"]
        .as_str()
        .filter(|s| !s.contains('/') && !s.contains(".."))
        .ok_or("Cannot identify the network interface for this display.")?;
    mac(
        std::fs::read_to_string(format!("/sys/class/net/{dev}/address"))
            .map_err(|e| e.to_string())?
            .trim(),
    )
}
#[cfg(not(target_os = "linux"))]
pub async fn controller_mac(_: &str) -> Result<String, String> {
    Err("Enter the controller MAC address to pair this VIDAA display.".into())
}
