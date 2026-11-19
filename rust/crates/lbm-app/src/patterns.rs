//! Own native OSD surfaces for one screen or a comparison group.
use lbm_pattern::{Pattern, Settings, Update};
use std::{
    io::{BufRead, Read, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

struct Running {
    child: Child,
    target: String,
    ready: bool,
    revision: u64,
    started: Instant,
    arrival: mpsc::Receiver<u64>,
}
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct Controller {
    running: Vec<Running>,
    settings: Option<Settings>,
    #[cfg(target_os = "linux")]
    editor: Option<editor::Guard>,
    raised: bool,
    pending: Option<Settings>,
    last_sent: Option<Instant>,
    pub error: Option<String>,
    /// Why the patterns cover the editor, when it cannot be kept above them.
    pub note: Option<String>,
}
impl Controller {
    pub fn supported() -> bool {
        cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
    }
    pub fn active(&self) -> Option<Pattern> {
        self.settings
            .as_ref()
            .filter(|_| !self.running.is_empty())
            .map(|s| s.pattern)
    }
    pub fn ready(&self) -> bool {
        !self.running.is_empty() && self.running.iter().all(|r| r.ready)
    }
    pub fn targets(&self) -> Vec<String> {
        self.running.iter().map(|r| r.target.clone()).collect()
    }
    pub fn stop(&mut self) {
        self.running.clear();
        #[cfg(target_os = "linux")]
        {
            self.editor = None;
        }
        self.raised = false;
        self.settings = None;
        self.pending = None;
        self.note = None;
    }
    pub fn show(&mut self, targets: &[String], settings: &Settings, ctx: &egui::Context) {
        if targets.is_empty() {
            self.stop();
            return;
        }
        if self.active().is_some() && self.targets() == targets {
            if self.settings.as_ref() != Some(settings) {
                self.pending = Some(settings.clone());
            } else {
                self.pending = None;
            }
            return;
        }
        // Retarget the OSDs without releasing the editor's keep-above state.
        // The caller supplies the current settings, including any pending slider edit.
        self.running.clear();
        self.pending = None;
        self.raised = false;
        self.error = None;
        self.note = None;
        self.settings = Some(settings.clone());
        let result = (|| -> Result<(), String> {
            // Only KWin can keep the editor above them. Elsewhere the patterns are shown
            // on their own, with the keyboard, and any key or click closes them.
            #[cfg(target_os = "linux")]
            if self.editor.is_none() {
                match editor::Guard::new(std::process::id()) {
                    Ok(guard) => self.editor = Some(guard),
                    Err(error) => {
                        self.note = Some(format!(
                            "{error}. The patterns cover the editor: press a key or click one to close them."
                        ))
                    }
                }
            }
            #[cfg(target_os = "linux")]
            let with_editor = self.editor.is_some();
            #[cfg(not(target_os = "linux"))]
            let with_editor = false;
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let config = serde_json::to_string(settings).map_err(|e| e.to_string())?;
            for target in targets {
                let mut command = Command::new(&exe);
                command.args(host_arguments(target, &config, with_editor));
                let mut child = command
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("Could not open pattern on {target}: {e}"))?;
                let stdout = child.stdout.take().expect("piped stdout");
                let (send, arrival) = mpsc::channel();
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    for line in std::io::BufReader::new(stdout).lines() {
                        let Ok(line) = line else { break };
                        if let Some(revision) =
                            line.strip_prefix("READY ").and_then(|s| s.parse().ok())
                        {
                            let _ = send.send(revision);
                            ctx.request_repaint();
                        }
                    }
                    ctx.request_repaint();
                });
                self.running.push(Running {
                    child,
                    target: target.clone(),
                    ready: false,
                    revision: 0,
                    started: Instant::now(),
                    arrival,
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.stop();
            self.error = Some(error);
        }
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        if self.running.is_empty() {
            return;
        }
        if self
            .last_sent
            .is_none_or(|at| at.elapsed() >= Duration::from_millis(120))
        {
            if let Some(settings) = self.pending.take() {
                let mut failed = None;
                for running in &mut self.running {
                    running.revision += 1;
                    let update = Update {
                        revision: running.revision,
                        settings: settings.clone(),
                    };
                    let mut json = serde_json::to_vec(&update).expect("pattern settings");
                    json.push(b'\n');
                    let result = running
                        .child
                        .stdin
                        .as_mut()
                        .expect("owner pipe")
                        .write_all(&json);
                    if let Err(error) = result {
                        failed = Some(error.to_string());
                        break;
                    }
                    running.ready = false;
                    running.started = Instant::now();
                }
                if let Some(error) = failed {
                    self.stop();
                    self.error = Some(format!("Could not update patterns: {error}"));
                    return;
                }
                self.settings = Some(settings);
                self.last_sent = Some(Instant::now());
            }
        }
        let mut ended = false;
        for running in &mut self.running {
            for revision in running.arrival.try_iter() {
                if revision == running.revision {
                    running.ready = true;
                }
            }
            match running.child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        let mut detail = String::new();
                        if let Some(stderr) = running.child.stderr.take() {
                            let _ = stderr.take(8192).read_to_string(&mut detail);
                        }
                        self.error = Some(format!(
                            "{}: {}",
                            running.target,
                            if detail.trim().is_empty() {
                                status.to_string()
                            } else {
                                detail.trim().into()
                            }
                        ));
                    }
                    ended = true;
                    break;
                }
                Ok(None)
                    if !running.ready && running.started.elapsed() > Duration::from_secs(15) =>
                {
                    self.error = Some(format!(
                        "{} did not accept the pattern surface.",
                        running.target
                    ));
                    ended = true;
                    break;
                }
                Err(error) => {
                    self.error = Some(error.to_string());
                    ended = true;
                    break;
                }
                _ => {}
            }
        }
        // Newly mapped OSDs occupy the same KWin layer as keep-above windows.
        // Raise only our editor once the whole group has mapped, not on slider edits.
        if !ended && !self.raised && self.ready() {
            #[cfg(target_os = "linux")]
            if let Some(editor) = &mut self.editor {
                if let Err(error) = editor.raise() {
                    self.error = Some(error);
                    ended = true;
                }
            }
            self.raised = true;
        }
        // Closing any comparison surface closes the group, including the reference.
        if ended {
            self.stop();
        } else {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
}

/// How one pattern surface is started, as a child of this editor.
fn host_arguments(target: &str, settings: &str, with_editor: bool) -> Vec<String> {
    let mut args = vec!["--test-pattern-host".to_owned(), "--owned".to_owned()];
    if with_editor {
        args.push("--with-editor".into());
    }
    args.extend([
        "--output".into(),
        target.into(),
        "--settings".into(),
        settings.into(),
    ]);
    args
}

#[cfg(target_os = "linux")]
pub mod editor;

#[cfg(test)]
mod tests {
    #[test]
    fn without_a_kept_above_editor_the_patterns_take_the_keyboard() {
        let with = super::host_arguments("DP-1", "{}", true);
        assert!(with.iter().any(|a| a == "--with-editor"));
        let without = super::host_arguments("DP-1", "{}", false);
        assert!(!without.iter().any(|a| a == "--with-editor"));
        assert_eq!(
            &without[without.len() - 4..],
            ["--output", "DP-1", "--settings", "{}"]
        );
    }
}
