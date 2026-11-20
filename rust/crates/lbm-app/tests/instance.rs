#![cfg(unix)]
use lbm_app::instance::{self, Address, Event, Inbox, Launch};
use serde_json::{json, Value};
use std::{
    thread,
    time::{Duration, Instant},
};

fn next(inbox: &Inbox) -> Event {
    let until = Instant::now() + Duration::from_secs(4);
    loop {
        let mut events = inbox.pending();
        if !events.is_empty() {
            assert_eq!(events.len(), 1);
            return events.remove(0);
        }
        assert!(Instant::now() < until, "editor command not received");
        thread::sleep(Duration::from_millis(5));
    }
}
fn open(dir: &std::path::Path) -> (instance::Owner, Inbox) {
    match instance::start(Address::in_directory(dir), false).unwrap() {
        Launch::Open {
            owner,
            inbox,
            draft,
        } => {
            assert!(draft.is_none());
            (owner, inbox)
        }
        Launch::Activated => panic!("first editor must open"),
    }
}
#[test]
fn a_second_launch_activates_the_owner_and_leaves_its_lock_held() {
    let dir = tempfile::tempdir().unwrap();
    let (owner, inbox) = open(dir.path());
    let path = dir.path().to_owned();
    let second =
        thread::spawn(move || instance::start(Address::in_directory(&path), false).unwrap());
    let Event::Activate(answer) = next(&inbox) else {
        panic!("must activate")
    };
    answer.send(Value::Null).unwrap();
    assert!(matches!(second.join().unwrap(), Launch::Activated));
    assert!(
        lbm_ipc::instance::InstanceLock::acquire(&dir.path().join("lbm-editor.lock"))
            .unwrap()
            .is_none()
    );
    drop(owner);
    assert!(!dir.path().join("lbm-editor.sock").exists());
    let _ = open(dir.path());
}
#[test]
fn replacement_carries_the_draft_and_waits_for_the_old_window_to_release_its_lock() {
    let dir = tempfile::tempdir().unwrap();
    let (owner, inbox) = open(dir.path());
    let path = dir.path().to_owned();
    let (done, result) = std::sync::mpsc::channel();
    let second = thread::spawn(move || {
        let launch = instance::start(Address::in_directory(&path), true).unwrap();
        done.send(launch).unwrap();
    });
    let Event::Replace(answer) = next(&inbox) else {
        panic!("must replace")
    };
    let draft = json!({"LayoutId":"test","Document":{"Layout":{"Options":{"LoopX":true}}}});
    answer.send(draft.clone()).unwrap();
    assert!(matches!(next(&inbox), Event::Close));
    assert!(
        result.recv_timeout(Duration::from_millis(100)).is_err(),
        "a second window opened before the first closed"
    );
    drop(owner);
    let Launch::Open {
        owner,
        inbox: _,
        draft: restored,
    } = result.recv_timeout(Duration::from_secs(2)).unwrap()
    else {
        panic!("replacement must open")
    };
    assert_eq!(restored, Some(draft));
    second.join().unwrap();
    drop(owner);
}
#[test]
fn simultaneous_launches_all_activate_one_owner() {
    let dir = tempfile::tempdir().unwrap();
    let (_owner, inbox) = open(dir.path());
    let children: Vec<_> = (0..8)
        .map(|_| {
            let path = dir.path().to_owned();
            thread::spawn(move || instance::start(Address::in_directory(&path), false).unwrap())
        })
        .collect();
    for _ in 0..children.len() {
        let Event::Activate(answer) = next(&inbox) else {
            panic!("must activate")
        };
        answer.send(Value::Null).unwrap();
    }
    for child in children {
        assert!(matches!(child.join().unwrap(), Launch::Activated));
    }
}
#[test]
fn a_crashed_owner_releases_the_lock_and_the_stale_socket_is_replaced() {
    const CHILD_DIR: &str = "LBM_INSTANCE_CRASH_TEST_DIR";
    if let Some(dir) = std::env::var_os(CHILD_DIR) {
        let (_owner, _inbox) = open(std::path::Path::new(&dir));
        std::fs::write(std::path::Path::new(&dir).join("ready"), b"ready").unwrap();
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "a_crashed_owner_releases_the_lock_and_the_stale_socket_is_replaced",
        ])
        .env(CHILD_DIR, dir.path())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let until = Instant::now() + Duration::from_secs(4);
    while !dir.path().join("ready").exists() {
        if Instant::now() >= until {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not start")
        }
        thread::sleep(Duration::from_millis(5));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(dir.path().join("lbm-editor.sock").exists());
    let (_owner, _inbox) = open(dir.path());
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(dir.path().join("lbm-editor.sock"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
