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
//! `rulers N` asks the remote to draw "Rulers on screens" on all its screens, measuring
//! its `N`th screen (0 first), as a master's editor computes them from the `Displays` it
//! was sent; `norulers` takes them down. What the remote says of them (`OverlayFailed`)
//! is printed as it comes.
//!
//! It hears of blocks as a master does (`InjectionBlocked`, a Windows remote whose
//! foreground window is elevated): each one is printed, and one said while the pointer
//! is over there brings it home as the agent would — a `Leave` — the script's steps
//! being skipped until its next `enter`.
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
    /// Rulers measuring the remote's screen of that index; `None` takes them down.
    Rulers(Option<usize>),
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
            "rulers" => {
                steps.push(Step::Rulers(Some(number(&mut words, "rulers")?)));
                continue;
            }
            "norulers" => {
                steps.push(Step::Rulers(None));
                continue;
            }
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
    let mut hello = Hello::new("peer_drive", env!("CARGO_PKG_VERSION"), Role::Master);
    hello.capabilities = vec![lbm_ipc::peer::INJECTION_BLOCKED_CAPABILITY.to_owned()];
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
    let mut screens = Vec::new();
    match displays {
        Ok(Ok((revision, displays))) => {
            let json = serde_json::json!({ "Revision": revision, "Displays": displays });
            println!(
                "{}",
                serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?
            );
            screens = displays;
        }
        Ok(Err(error)) => return Err(error),
        Err(_) => println!("no Displays within {DISPLAYS_PATIENCE:?}; playing the script anyway"),
    }

    // What the remote says from now on, read beside the script.
    let started = std::time::Instant::now();
    let (blocks, mut blocked) = tokio::sync::mpsc::unbounded_channel();
    let reader = tokio::spawn(async move {
        while let Some(event) = link.next().await {
            match event {
                Event::Control(Control::InjectionBlocked { block }) => {
                    println!(
                        "<- {:>6} ms InjectionBlocked {block:?}",
                        started.elapsed().as_millis()
                    );
                    let _ = blocks.send(block.is_some());
                }
                Event::Control(Control::InputReset) => {
                    println!("<- {:>6} ms InputReset", started.elapsed().as_millis());
                }
                Event::Control(Control::OverlayFailed { revision, error }) => {
                    println!(
                        "<- {:>6} ms rulers {revision} not shown: {error}",
                        started.elapsed().as_millis()
                    );
                }
                Event::Closed(_) => break,
                _ => {}
            }
        }
    });
    // The pointer is over there: an `Enter` sent and no `Leave` since.
    let mut over_there = false;
    // The last `Overlay` sent.
    let mut revision = 0;
    let come_home = |block: bool, over_there: &mut bool| -> bool {
        let home = block && *over_there;
        if home {
            println!("   blocked over there: the pointer comes home");
            *over_there = false;
        }
        home
    };
    for step in steps {
        while let Ok(block) = blocked.try_recv() {
            if come_home(block, &mut over_there) {
                send(&sender, vec![InputEvent::Leave], started).await?;
            }
        }
        match step {
            Step::Send(events) => {
                let enters = matches!(events.first(), Some(InputEvent::Enter { .. }));
                if !over_there && !enters && events != [InputEvent::Leave] {
                    println!("   skipped (the pointer is home): {events:?}");
                    continue;
                }
                over_there = enters || (over_there && events != [InputEvent::Leave]);
                send(&sender, events, started).await?;
            }
            Step::Sleep(duration) => {
                let deadline = tokio::time::Instant::now() + duration;
                while let Ok(Some(block)) = tokio::time::timeout_at(deadline, blocked.recv()).await
                {
                    if come_home(block, &mut over_there) {
                        send(&sender, vec![InputEvent::Leave], started).await?;
                    }
                }
            }
            Step::Rulers(measured) => {
                let bands = match measured {
                    Some(index) => rulers(&peer.fingerprint, &peer.hello.name, &screens, index)?,
                    None => Vec::new(),
                };
                revision += 1;
                println!(
                    "-> {:>6} ms Overlay {revision}: {} band(s)",
                    started.elapsed().as_millis(),
                    bands.len()
                );
                sender
                    .control(&Control::Overlay { revision, bands })
                    .await
                    .map_err(|e| format!("cannot send: {e}"))?;
            }
        }
    }
    sender.close("the script is over").await;
    // The Bye leaves before the link reports itself closed.
    let _ = tokio::time::timeout(Duration::from_secs(2), reader).await;
    println!("bye");
    Ok(())
}

async fn send(
    sender: &link::LinkSender,
    events: Vec<InputEvent>,
    started: std::time::Instant,
) -> Result<(), String> {
    println!("-> {:>6} ms {events:?}", started.elapsed().as_millis());
    sender
        .input(&encode(&events))
        .await
        .map_err(|e| format!("cannot send: {e}"))
}

/// "Rulers on screens" for the remote's screens, measuring the `index`th, as a master's
/// editor computes them: its screens merged into a layout of their own, placed as the
/// remote places them, and `lbm_overlay::scene::from_layout`.
fn rulers(
    machine_id: &str,
    name: &str,
    screens: &[lbm_ipc::peer::PeerDisplay],
    index: usize,
) -> Result<Vec<serde_json::Value>, String> {
    let mut layout = lbm_layout::model::Layout::new(Default::default());
    lbm_display::peer::merge_peer(&mut layout, machine_id, name, screens);
    layout.set_locations_from_system_configuration(false);
    layout.anchor_on_primary();
    let measured = layout
        .monitors()
        .get(index)
        .map(|m| m.id.clone())
        .ok_or_else(|| format!("the remote has no screen {index}"))?;
    let bands =
        lbm_overlay::scene::from_layout(&layout, lbm_overlay::Mode::Rulers, Some(&measured))?;
    bands
        .iter()
        .map(|band| serde_json::to_value(band).map_err(|e| e.to_string()))
        .collect()
}
