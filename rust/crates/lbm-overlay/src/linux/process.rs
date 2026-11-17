use super::{Command, Reply, MAX_MESSAGE};
use crate::{scene::Band, Backend};
use std::{
    io::{BufRead, Read, Write},
    process::{Child, Stdio},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};
#[derive(Default)]
struct State {
    latest: Option<Vec<Band>>,
    pulse: bool,
    stop: bool,
    error: Option<String>,
}
type Shared = Arc<(Mutex<State>, Condvar)>;
struct Session {
    child: Child,
    shared: Shared,
    writer: Option<JoinHandle<()>>,
    reader: Option<JoinHandle<()>>,
}
impl Session {
    fn start(mut command: std::process::Command) -> Result<Self, String> {
        // Never share the parent toolkit's inherited Wayland connection with the helper.
        let mut child = command
            .env_remove("WAYLAND_SOCKET")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Cannot start screen overlays: {e}"))?;
        let mut input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let send = shared.clone();
        let writer = std::thread::spawn(move || loop {
            let (mutex, notify) = &*send;
            let command = {
                let mut s = mutex.lock().unwrap();
                while !s.stop && !s.pulse && s.latest.is_none() {
                    s = notify.wait(s).unwrap();
                }
                if s.stop {
                    return;
                }
                s.pulse = false;
                s.latest
                    .take()
                    .map(Command::Scene)
                    .unwrap_or(Command::Pulse)
            };
            let result = (|| -> Result<(), String> {
                let mut bytes = serde_json::to_vec(&command).map_err(|e| e.to_string())?;
                bytes.push(b'\n');
                if bytes.len() > MAX_MESSAGE {
                    return Err("Overlay scene exceeds its size limit.".into());
                }
                input.write_all(&bytes).map_err(|e| e.to_string())
            })();
            if let Err(error) = result {
                mutex.lock().unwrap().error.get_or_insert(error);
                return;
            }
        });
        let receive = shared.clone();
        let reader = std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(output);
            loop {
                let mut line = Vec::new();
                match (&mut reader).take(4097).read_until(b'\n', &mut line) {
                    Ok(0) => return,
                    Ok(n) if n <= 4096 => match serde_json::from_slice::<Reply>(&line) {
                        Ok(Reply::Ready) => {}
                        Ok(Reply::Error(error)) => {
                            receive.0.lock().unwrap().error = Some(error);
                            return;
                        }
                        Err(e) => {
                            receive.0.lock().unwrap().error =
                                Some(format!("Invalid overlay reply: {e}"));
                            return;
                        }
                    },
                    _ => {
                        receive.0.lock().unwrap().error =
                            Some("Overlay response failed or exceeded its size limit.".into());
                        return;
                    }
                }
            }
        });
        Ok(Self {
            child,
            shared,
            writer: Some(writer),
            reader: Some(reader),
        })
    }
    fn poll(&mut self) -> Result<(), String> {
        if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
            return Err(self
                .shared
                .0
                .lock()
                .unwrap()
                .error
                .take()
                .unwrap_or_else(|| format!("Screen overlays stopped ({status}).")));
        }
        let mut state = self.shared.0.lock().unwrap();
        if let Some(error) = state.error.take() {
            return Err(error);
        }
        state.pulse = true;
        self.shared.1.notify_one();
        Ok(())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.shared.0.lock().unwrap().stop = true;
        self.shared.1.notify_one();
        // Killing the owned helper closes its Wayland/X connection and unblocks both pipes.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(writer) = self.writer.take() {
            let _ = writer.join();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
#[derive(Default)]
pub struct Native {
    session: Option<Session>,
}
impl Backend for Native {
    fn present(&mut self, bands: &[Band]) -> Result<(), String> {
        super::validate(bands)?;
        if self.session.is_none() {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let mut command = std::process::Command::new(exe);
            command.arg("--screen-overlay-host");
            self.session = Some(Session::start(command)?);
        }
        let session = self.session.as_mut().unwrap();
        session.poll()?;
        session.shared.0.lock().unwrap().latest = Some(bands.to_vec());
        session.shared.1.notify_one();
        Ok(())
    }
    fn clear(&mut self) {
        self.session = None;
    }
    fn poll(&mut self) -> Result<(), String> {
        if let Some(s) = &mut self.session {
            s.poll()
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn helper(script: &str) -> Session {
        let mut command = std::process::Command::new("/bin/sh");
        command.arg("-c").arg(script);
        Session::start(command).unwrap()
    }
    fn error(session: &mut Session) -> String {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if let Err(e) = session.poll() {
                return e;
            }
            assert!(Instant::now() < until, "helper did not answer");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn native_errors_survive_child_exit_and_are_reported_to_the_editor() {
        let mut s = helper("read line; printf '%s\\n' '{\"Error\":\"missing layer-shell\"}'");
        assert_eq!(error(&mut s), "missing layer-shell");
    }
    #[test]
    fn malformed_and_oversized_responses_fail_closed() {
        for script in [
            "read line; printf '%s\\n' invalid",
            "read line; i=0; while [ $i -lt 4100 ]; do printf x; i=$((i+1)); done; printf '\\n'",
        ] {
            let mut s = helper(script);
            let e = error(&mut s);
            assert!(
                e.contains("overlay reply") || e.contains("size limit"),
                "{e}"
            );
        }
    }
    #[test]
    fn dropping_a_stalled_helper_kills_and_reaps_it_without_waiting_for_native_io() {
        let mut s = helper("read line; exec sleep 60");
        s.poll().unwrap();
        let pid = s.child.id();
        let start = Instant::now();
        drop(s);
        assert!(start.elapsed() < Duration::from_secs(1));
        // SAFETY: signal zero only checks existence; waitpid in Drop already reaped this child.
        assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ESRCH)
        );
    }
    #[test]
    fn environment_does_not_reuse_the_editors_inherited_wayland_socket() {
        let mut command = std::process::Command::new("/bin/sh");
        command.env("WAYLAND_SOCKET","123").arg("-c").arg("read line; if [ -n \"${WAYLAND_SOCKET+x}\" ]; then printf '%s\\n' '{\"Error\":\"inherited\"}'; else printf '%s\\n' '{\"Error\":\"isolated\"}'; fi");
        assert_eq!(error(&mut Session::start(command).unwrap()), "isolated");
    }
}
