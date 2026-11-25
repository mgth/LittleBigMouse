//! A master of a test's own, for the trial of the service on a real machine: connects to
//! the service like an agent would, as a master, and sends a script of input.
//! Not shipped.
//!
//! ```text
//! master_trial <identity-dir> --id                              this master's fingerprint
//! master_trial <identity-dir> <host:port> <service-id> <script> plays the script
//! ```
//! The service must answer with the fingerprint `service-id` (as published under
//! `HKLM\SOFTWARE\LittleBigMouse\InputService`): another machine at that address
//! gets nothing. A send that fails fails the trial.
//!
//! Script: `enter X Y`, `move X Y`, `key CODE down|up`, `button CODE down|up`,
//! `sleep MS`, `leave`, `hold MS` (keeps the link, printing what the service says),
//! `expect-close MS` (the service must close the link within that time, as a
//! withdrawal does; the script ends there).

use std::time::Duration;

use lbm_ipc::input::{encode, InputEvent};
use lbm_net::identity::Identity;
use lbm_net::link::{self, Event};
use lbm_net::wire::{Hello, Role};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let usage = "master_trial <identity-dir> (--id | <host:port> <service-id> <script>)";
    let directory = arguments.next().ok_or(usage)?;
    let identity = Identity::load_or_create(std::path::Path::new(&directory))
        .map_err(|e| format!("identity: {e}"))?;
    let target = arguments.next().ok_or(usage)?;
    if target == "--id" {
        println!("{}", identity.fingerprint());
        return Ok(());
    }
    let expected = arguments.next().ok_or(usage)?;
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
    let found = pending.peer().fingerprint.clone();
    println!("service {} ({found})", pending.peer().hello.name);
    if found != expected {
        pending.refuse("not the service this trial is for").await;
        return Err(format!(
            "{target} answered as {found}, not {expected}: nothing sent"
        ));
    }
    let link = pending.accept().await.map_err(|e| format!("accept: {e}"))?;
    let (_, sender, mut events) = link.into_parts();
    let (closed_tx, mut closed) = tokio::sync::watch::channel(false);
    tokio::spawn(async move {
        while let Some(event) = events.recv().await {
            println!("{} said: {event:?}", chrono_like());
            if matches!(event, Event::Closed(_)) {
                break;
            }
        }
        let _ = closed_tx.send(true);
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
            Some("expect-close") => {
                let within = Duration::from_millis(number(1)? as u64);
                return match tokio::time::timeout(within, closed.wait_for(|c| *c)).await {
                    Ok(_) => {
                        println!("{} closed by the service, as expected", chrono_like());
                        Ok(())
                    }
                    Err(_) => Err(format!("the service did not close the link: {line}")),
                };
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
        sender
            .input(&encode(&[event]))
            .await
            .map_err(|error| format!("send failed at `{line}`: {error:?}"))?;
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
