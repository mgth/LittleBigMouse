//! A master of several remotes at once, for trying "Several remotes at once"
//! (docs/multi-pc.md) end to end without a hook on this side.
//!
//! ```text
//! cargo run -p lbm-agent --example multi_master -- pair MASTER_DIR REMOTE_DIR ADDR
//! cargo run -p lbm-agent --example multi_master -- drive MASTER_DIR PORT COUNT
//! ```
//!
//! `pair` makes an identity in each directory if there is none and pairs them as if both
//! users had accepted: the master's `peers.json` names the remote at `ADDR` (`host:port`),
//! the remote's names the master. Run it once per remote; `REMOTE_DIR` is the remote
//! agent's configuration directory (`--config-dir`), whose `multi-pc.json` it turns on at
//! `ADDR`'s port.
//!
//! `drive` runs the agent's own multi-PC manager (`lbm_agent::peers::Service`) with the
//! mode on at `PORT`, as the master of a hook that names its machines, and plays that
//! hook itself on the input endpoint: once `COUNT` remotes are linked and drivable, it
//! crosses onto the first, moves, types and clicks there, goes straight to the next one
//! with a `Leave` and an `Enter` in one batch, and so on, then back to the first, and
//! home. It prints the state's `Driven` after every crossing. Pointer positions are the
//! remotes' own pixels: the middle of each one's first screen.
//!
//! It opens no input device, injects nothing and holds no endpoint of the user's agent
//! or hook; give it a port nobody uses.

#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::ExitCode;
#[cfg(unix)]
use std::time::Duration;

#[cfg(unix)]
use lbm_agent::peers::{Answer, Notice, PeerRequest, PeersState, Service, Setup};
#[cfg(unix)]
use lbm_ipc::input::{encode, encode_host, write_batch, InputEvent};
#[cfg(unix)]
use lbm_net::identity::Identity;
#[cfg(unix)]
use lbm_net::peers::{PairedPeer, PeerStore};
#[cfg(unix)]
use lbm_net::wire::Role;

#[cfg(unix)]
const PATIENCE: Duration = Duration::from_secs(30);
#[cfg(unix)]
const KEY_A: u16 = 30;
#[cfg(unix)]
const BTN_LEFT: u16 = 0x110;

#[cfg(unix)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["pair", master, remote, address] => pair(Path::new(master), Path::new(remote), address),
        ["drive", master, port, count] => match (port.parse(), count.parse()) {
            (Ok(port), Ok(count)) => drive(PathBuf::from(master), port, count).await,
            _ => Err("PORT and COUNT are numbers".to_owned()),
        },
        _ => Err(
            "usage: multi_master pair MASTER_DIR REMOTE_DIR ADDR | drive MASTER_DIR PORT COUNT"
                .to_owned(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("multi_master: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(unix)]
fn pair(master: &Path, remote: &Path, address: &str) -> Result<(), String> {
    let port: u16 = address
        .rsplit_once(':')
        .and_then(|(_, port)| port.parse().ok())
        .ok_or("ADDR is host:port")?;
    for directory in [master, remote] {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    }
    let master_id = Identity::load_or_create(master).map_err(|e| e.to_string())?;
    let remote_id = Identity::load_or_create(remote).map_err(|e| e.to_string())?;
    PeerStore::load(master)
        .insert(PairedPeer {
            machine_id: remote_id.fingerprint().to_owned(),
            name: format!("remote {port}"),
            address: address.to_owned(),
            role: Role::Remote,
        })
        .map_err(|e| e.to_string())?;
    PeerStore::load(remote)
        .insert(PairedPeer {
            machine_id: master_id.fingerprint().to_owned(),
            name: "multi_master".to_owned(),
            address: String::new(),
            role: Role::Master,
        })
        .map_err(|e| e.to_string())?;
    let settings = serde_json::json!({ "Enabled": true, "Port": port });
    std::fs::write(
        remote.join("multi-pc.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    println!("master {}", master_id.fingerprint());
    println!("remote {} at {address}", remote_id.fingerprint());
    Ok(())
}

/// The latest state, or the next one announced, that satisfies `want`.
#[cfg(unix)]
async fn until(
    notices: &mut tokio::sync::mpsc::UnboundedReceiver<Notice>,
    last: &mut Option<PeersState>,
    want: impl Fn(&PeersState) -> bool,
) -> Result<PeersState, String> {
    while let Ok(notice) = notices.try_recv() {
        if let Notice::Peers(state) = notice {
            *last = Some(state);
        }
    }
    if let Some(state) = last.clone().filter(|s| want(s)) {
        return Ok(state);
    }
    tokio::time::timeout(PATIENCE, async {
        while let Some(notice) = notices.recv().await {
            if let Notice::Peers(state) = notice {
                *last = Some(state.clone());
                if want(&state) {
                    return Some(state);
                }
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
    .ok_or_else(|| "the state never got there".to_owned())
}

#[cfg(unix)]
async fn drive(directory: PathBuf, port: u16, count: usize) -> Result<(), String> {
    let endpoint = directory.join("master-input.sock");
    let _ = std::fs::remove_file(&endpoint);
    let hook = tokio::net::UnixListener::bind(&endpoint).map_err(|e| e.to_string())?;
    let mut service = Service::new(
        directory.clone(),
        Setup::new(Some(endpoint.to_string_lossy().into_owned())),
    );
    let mut notices = service.take_notices().expect("the notices, once");
    service.hook_names_hosts(true);
    let on = service.request(PeerRequest::Set {
        enabled: true,
        port: Some(port),
        name: Some("multi_master".to_owned()),
        pointer_lock_shortcut: None,
        clipboard: Default::default(),
    });
    match on {
        Answer::Now(result) => result,
        Answer::Later(result) => result.await,
    }?;
    let mut last = None;
    let state = until(&mut notices, &mut last, |s| {
        s.drivable.len() >= count && s.remote_screens().len() >= count
    })
    .await?;
    println!(
        "drives {} remote(s), selected {:?}, the hook is told Drivable {:?}",
        state.drivable.len(),
        state.selected_remote,
        state.drivable
    );
    let remotes = state.remote_screens();
    for remote in &remotes {
        let screen = &remote.displays[0];
        println!(
            "  {} ({}): {} screen(s), the first {}x{} at ({},{})",
            remote.name,
            remote.machine_id,
            remote.displays.len(),
            screen.width,
            screen.height,
            screen.x,
            screen.y
        );
    }

    let (mut stream, _) = tokio::time::timeout(PATIENCE, hook.accept())
        .await
        .map_err(|_| "the manager never connected to the input endpoint")?
        .map_err(|e| e.to_string())?;
    println!("the manager connected to this hook's input endpoint");

    let middle = |i: usize| {
        let screen = &remotes[i].displays[0];
        (screen.x + screen.width / 2, screen.y + screen.height / 2)
    };
    // Onto each one in turn, then back to the first, then home.
    let mut order: Vec<usize> = (0..remotes.len()).collect();
    order.push(0);
    let mut from: Option<usize> = None;
    for &i in &order {
        let (x, y) = middle(i);
        let mut batch = Vec::new();
        if from.is_some() {
            InputEvent::Leave.encode(&mut batch);
        }
        encode_host(&mut batch, &remotes[i].machine_id);
        batch.extend_from_slice(&encode(&[InputEvent::Enter { x, y }]));
        write_batch(&mut stream, &batch)
            .await
            .map_err(|e| e.to_string())?;
        for step in 1..=5 {
            let motion = encode(&[InputEvent::Motion {
                x: x + step * 10,
                y,
            }]);
            write_batch(&mut stream, &motion)
                .await
                .map_err(|e| e.to_string())?;
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let typed = encode(&[
            InputEvent::Key {
                code: KEY_A,
                pressed: true,
            },
            InputEvent::Key {
                code: KEY_A,
                pressed: false,
            },
            InputEvent::Button {
                code: BTN_LEFT,
                pressed: true,
            },
            InputEvent::Button {
                code: BTN_LEFT,
                pressed: false,
            },
        ]);
        write_batch(&mut stream, &typed)
            .await
            .map_err(|e| e.to_string())?;
        let id = remotes[i].machine_id.clone();
        let now = until(&mut notices, &mut last, |s| {
            s.driven.as_deref() == Some(&id)
        })
        .await?;
        println!(
            "{} -> {}: Driven = {:?}",
            from.map_or("here".to_owned(), |f| remotes[f].name.clone()),
            remotes[i].name,
            now.driven
        );
        from = Some(i);
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    write_batch(&mut stream, &encode(&[InputEvent::Leave]))
        .await
        .map_err(|e| e.to_string())?;
    let home = until(&mut notices, &mut last, |s| s.driven.is_none()).await?;
    println!("home: Driven = {:?}", home.driven);
    tokio::time::sleep(Duration::from_millis(300)).await;
    service.stop().await;
    println!("stopped");
    Ok(())
}

/// The master plays its hook on a Unix socket.
#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("multi_master: Unix only");
    std::process::ExitCode::FAILURE
}
