//! A trial of the service's launch path without the service, for a test machine:
//! run as SYSTEM (`qm guest exec`, a SYSTEM scheduled task), it launches `lbm-inject`
//! into the console session exactly as the service does, then writes a script of
//! input to it. What it shows: whether a SYSTEM injector moved into the session
//! reaches the secure desktop (UAC prompt, lock screen) and elevated windows.
//!
//! Not shipped. Only SYSTEM can move a token into a session, so run by anyone else it
//! fails at the launch.
//!
//! ```text
//! inject_trial <lbm-inject.exe> <script> [<log>]
//! ```
//!
//! One command per line: `enter X Y`, `move X Y`, `key CODE down|up`,
//! `button CODE down|up`, `sleep MS`, `leave`. Codes are Linux ones
//! (`KEY_ESC` = 1, `KEY_SPACE` = 57, `BTN_LEFT` = 272).

#[cfg(windows)]
fn main() -> Result<(), String> {
    use std::io::Write;

    use lbm_ipc::input::{encode, InputEvent};

    let mut arguments = std::env::args().skip(1);
    let usage = "inject_trial <lbm-inject.exe> <script> [<log>]";
    let injector = std::path::PathBuf::from(arguments.next().ok_or(usage)?);
    let script = std::fs::read_to_string(arguments.next().ok_or(usage)?)
        .map_err(|e| format!("script: {e}"))?;
    let log = arguments
        .next()
        .map(std::fs::File::create)
        .transpose()
        .map_err(|e| format!("log: {e}"))?;

    let session = lbm_service::win::launch::console_session().ok_or("no console session")?;
    let mut started = lbm_service::win::launch::launch(&injector, session, log.as_ref())?;
    println!("injector {} in session {session}", started.pid);

    for (number, line) in script.lines().enumerate() {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number_at = |index: usize| -> Result<i32, String> {
            words
                .get(index)
                .and_then(|word| word.parse().ok())
                .ok_or_else(|| format!("line {}: {line}", number + 1))
        };
        let pressed = || words.get(2) == Some(&"down");
        let event = match words.first().copied() {
            None => continue,
            Some(word) if word.starts_with('#') => continue,
            Some("sleep") => {
                std::thread::sleep(std::time::Duration::from_millis(number_at(1)? as u64));
                continue;
            }
            Some("enter") => InputEvent::Enter {
                x: number_at(1)?,
                y: number_at(2)?,
            },
            Some("move") => InputEvent::Motion {
                x: number_at(1)?,
                y: number_at(2)?,
            },
            Some("key") => InputEvent::Key {
                code: number_at(1)? as u16,
                pressed: pressed(),
            },
            Some("button") => InputEvent::Button {
                code: number_at(1)? as u16,
                pressed: pressed(),
            },
            Some("leave") => InputEvent::Leave,
            Some(_) => return Err(format!("line {}: {line}", number + 1)),
        };
        let batch = encode(&[event]);
        let mut frame = (batch.len() as u32).to_le_bytes().to_vec();
        frame.extend_from_slice(&batch);
        started
            .input
            .write_all(&frame)
            .map_err(|e| format!("the injector stopped reading: {e}"))?;
    }
    started.stop(std::time::Duration::from_secs(2));
    println!("done");
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("inject_trial only runs on Windows");
}
