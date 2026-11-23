//! A scripted master, for trying a remote end to end without a hook on this side.
//!
//! ```text
//! cargo run -p lbm-agent --example peer_drive -- ADDR IDENTITY_DIR [SCRIPT…]
//! cargo run -p lbm-agent --example peer_drive -- 192.168.1.30 ./drive-id \
//!     enter 100 100  move 500 300  click left  key 30  wheel 0 120  sleep 200  leave
//! ```
//!
//! It connects to `ADDR` (the default port when none is given) as a master, with the
//! identity kept in `IDENTITY_DIR` (made there the first time: keep the directory to
//! stay paired), prints its own fingerprint and the remote's, and waits for the pairing
//! — accept it on the remote, which shows this fingerprint. It then prints the
//! `Displays` the remote sends, as JSON, plays the script and says `Bye`.
//!
//! Script, positions in the remote's desktop pixels and codes as Linux input codes:
//! `enter X Y`, `move X Y`, `leave`, `click|press|release left|right|middle|CODE`,
//! `key|keydown|keyup CODE`, `wheel H V` (1/120ths of a notch), `sleep MS`.
//!
//! It opens no input device and drives nothing on this machine: everything it sends is
//! typed on the command line.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use lbm_ipc::input::{encode, InputEvent};
use lbm_net::identity::Identity;
use lbm_net::link::{self, Event};
use lbm_net::wire::{Control, Hello, Role};

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

/// How long the remote has to say where its screens are before the script runs anyway.
const DISPLAYS_PATIENCE: Duration = Duration::from_secs(10);

enum Step {
    Send(Vec<InputEvent>),
    Sleep(Duration),
}

fn number<T: std::str::FromStr>(
    words: &mut impl Iterator<Item = String>,
    what: &str,
) -> Result<T, String> {
    let word = words
        .next()
        .ok_or_else(|| format!("{what}: a number is missing"))?;
    word.parse()
        .map_err(|_| format!("{what}: {word} is not a number"))
}

fn button(word: Option<String>) -> Result<u16, String> {
    let word = word.ok_or("a button is missing")?;
    match word.as_str() {
        "left" => Ok(BTN_LEFT),
        "right" => Ok(BTN_RIGHT),
        "middle" => Ok(BTN_MIDDLE),
        code => code
            .parse()
            .map_err(|_| format!("{code} is neither left, right, middle nor a code")),
    }
}

fn script(words: Vec<String>) -> Result<Vec<Step>, String> {
    let mut words = words.into_iter();
    let mut steps = Vec::new();
    while let Some(word) = words.next() {
        let events = match word.as_str() {
            "enter" => vec![InputEvent::Enter {
                x: number(&mut words, "enter")?,
                y: number(&mut words, "enter")?,
            }],
            "move" => vec![InputEvent::Motion {
                x: number(&mut words, "move")?,
                y: number(&mut words, "move")?,
            }],
            "leave" => vec![InputEvent::Leave],
            "click" | "press" | "release" => {
                let code = button(words.next())?;
                let press = InputEvent::Button {
                    code,
                    pressed: true,
                };
                let release = InputEvent::Button {
                    code,
                    pressed: false,
                };
                match word.as_str() {
                    "click" => vec![press, release],
                    "press" => vec![press],
                    _ => vec![release],
                }
            }
            "key" | "keydown" | "keyup" => {
                let code = number(&mut words, &word)?;
                let down = InputEvent::Key {
                    code,
                    pressed: true,
                };
                let up = InputEvent::Key {
                    code,
                    pressed: false,
                };
                match word.as_str() {
                    "key" => vec![down, up],
                    "keydown" => vec![down],
                    _ => vec![up],
                }
            }
            "wheel" => vec![InputEvent::Wheel {
                horizontal: number(&mut words, "wheel")?,
                vertical: number(&mut words, "wheel")?,
            }],
            "sleep" => {
                steps.push(Step::Sleep(Duration::from_millis(number(
                    &mut words, "sleep",
                )?)));
                continue;
            }
            other => return Err(format!("unknown step {other}")),
        };
        steps.push(Step::Send(events));
    }
    Ok(steps)
}

fn with_default_port(address: String) -> String {
    if address.parse::<std::net::SocketAddr>().is_ok() || address.contains(':') {
        address
    } else {
        format!("{address}:{}", lbm_net::DEFAULT_PORT)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(address), Some(directory)) = (args.next(), args.next()) else {
        eprintln!("usage: peer_drive ADDR IDENTITY_DIR [SCRIPT…]");
        return ExitCode::FAILURE;
    };
    let steps = match script(args.collect()) {
        Ok(steps) => steps,
        Err(error) => {
            eprintln!("peer_drive: {error}");
            return ExitCode::FAILURE;
        }
    };
    match drive(with_default_port(address), PathBuf::from(directory), steps).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("peer_drive: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn drive(address: String, directory: PathBuf, steps: Vec<Step>) -> Result<(), String> {
    let identity = Identity::load_or_create(&directory).map_err(|e| e.to_string())?;
    println!("this machine: {}", identity.fingerprint());
    let hello = Hello::new("peer_drive", env!("CARGO_PKG_VERSION"), Role::Master);
    let options = link::Options::default();
    let pending = link::connect(address.as_str(), &identity, hello, options)
        .await
        .map_err(|e| format!("cannot connect to {address}: {e}"))?;
    let peer = pending.peer().clone();
    println!(
        "remote: {} ({}, {}) {}",
        peer.hello.name, peer.hello.os, peer.hello.version, peer.fingerprint
    );
    println!("waiting for the remote to accept the pairing…");
    let mut link = pending
        .accept()
        .await
        .map_err(|e| format!("not paired: {e}"))?;
    println!("paired");
    let sender = link.sender();

    let displays = tokio::time::timeout(DISPLAYS_PATIENCE, async {
        loop {
            match link.next().await {
                Some(Event::Control(Control::Displays { revision, displays })) => {
                    return Ok((revision, displays))
                }
                Some(Event::Closed(reason)) => return Err(format!("the link ended: {reason:?}")),
                None => return Err("the link ended".to_owned()),
                Some(_) => {}
            }
        }
    })
    .await;
    match displays {
        Ok(Ok((revision, displays))) => {
            let json = serde_json::json!({ "Revision": revision, "Displays": displays });
            println!(
                "{}",
                serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?
            );
        }
        Ok(Err(error)) => return Err(error),
        Err(_) => println!("no Displays within {DISPLAYS_PATIENCE:?}; playing the script anyway"),
    }

    for step in steps {
        match step {
            Step::Send(events) => {
                println!("-> {events:?}");
                sender
                    .input(&encode(&events))
                    .await
                    .map_err(|e| format!("cannot send: {e}"))?;
            }
            Step::Sleep(duration) => tokio::time::sleep(duration).await,
        }
    }
    sender.close("the script is over").await;
    // The Bye leaves before the link reports itself closed.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(event) = link.next().await {
            if let Event::Closed(_) = event {
                break;
            }
        }
    })
    .await;
    println!("bye");
    Ok(())
}
