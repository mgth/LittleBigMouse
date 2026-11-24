//! A master of a test's own, for the trial of the service on a real machine: connects to
//! the service like an agent would, as a master, and sends a script of input.
//! Not shipped.
//!
//! ```text
//! master_trial <identity-dir> --id                 prints this master's fingerprint
//! master_trial <identity-dir> <host:port> <script> plays the script
//! ```
//! Script: `enter X Y`, `move X Y`, `key CODE down|up`, `button CODE down|up`,
//! `sleep MS`, `leave`, `hold MS` (keeps the link, printing what the service says).

use std::time::Duration;

use lbm_ipc::input::{encode, InputEvent};
use lbm_net::identity::Identity;
use lbm_net::link::{self, Event};
use lbm_net::wire::{Hello, Role};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let usage = "master_trial <identity-dir> (--id | <host:port> <script>)";
    let directory = arguments.next().ok_or(usage)?;
    let identity = Identity::load_or_create(std::path::Path::new(&directory))
        .map_err(|e| format!("identity: {e}"))?;
    let target = arguments.next().ok_or(usage)?;
    if target == "--id" {
        println!("{}", identity.fingerprint());
        return Ok(());
    }
    let script = std::fs::read_to_string(arguments.next().ok_or(usage)?)
        .map_err(|e| format!("script: {e}"))?;
    let pending = link::connect(
        target.as_str(),
        &identity,
        Hello::new("trial-master", "0", Role::Master),
        link::Options::default(),
    )
    .await
    .map_err(|e| format!("connect: {e}"))?;
    println!(
        "service {} ({})",
        pending.peer().hello.name,
        pending.peer().fingerprint
    );
    let link = pending.accept().await.map_err(|e| format!("accept: {e}"))?;
    let (_, sender, mut events) = link.into_parts();
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            println!("{} said: {event:?}", chrono_like());
            if matches!(event, Event::Closed(_)) {
                break;
            }
        }
    });
    for line in script.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |i: usize| -> Result<i32, String> {
            words
                .get(i)
                .and_then(|w| w.parse().ok())
                .ok_or_else(|| format!("bad line: {line}"))
        };
        let pressed = words.get(2) == Some(&"down");
        let event = match words.first().copied() {
            None => continue,
            Some(w) if w.starts_with('#') => continue,
            Some("sleep") | Some("hold") => {
                tokio::time::sleep(Duration::from_millis(number(1)? as u64)).await;
                continue;
            }
            Some("enter") => InputEvent::Enter {
                x: number(1)?,
                y: number(2)?,
            },
            Some("move") => InputEvent::Motion {
                x: number(1)?,
                y: number(2)?,
            },
            Some("key") => InputEvent::Key {
                code: number(1)? as u16,
                pressed,
            },
            Some("button") => InputEvent::Button {
                code: number(1)? as u16,
                pressed,
            },
            Some("leave") => InputEvent::Leave,
            Some(_) => return Err(format!("bad line: {line}")),
        };
        if let Err(error) = sender.input(&encode(&[event])).await {
            println!("{} send failed: {error:?}", chrono_like());
            break;
        }
    }
    sender.close("done").await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    println!("{} done", chrono_like());
    Ok(())
}

fn chrono_like() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("[{}.{:03}]", now.as_secs() % 100_000, now.subsec_millis())
}
