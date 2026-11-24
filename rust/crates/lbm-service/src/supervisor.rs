//! Which session has an injector: the one at the console, and only while a stream
//! wants one.
//!
//! An injector is launched on demand, when a master's stream starts, into the session
//! then at the console — before anyone logs on there is one too, the logon screen's.
//! When the console moves to another session (fast user switching, a logoff, a remote
//! session taking the console), the injector of the one it left is stopped: what it
//! injects would land on a screen nobody sees, and ending its input is what releases
//! whatever the stream held pressed there. Sessions that are not at the console (RDP)
//! never get one.
//!
//! Everything here is a decision on what the service was told; the service does what
//! it says ([`Action`]) and reports back ([`Supervisor::exited`]).

/// What the service is to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Launch an injector into this session.
    Launch(u32),
    /// Stop the injector of this session: close its input, which makes it release
    /// what is pressed and leave.
    Stop(u32),
}

/// The state the decisions are taken on.
#[derive(Debug, Default)]
pub struct Supervisor {
    /// The session at the console; `None` while the console is attached to none
    /// (between a disconnect and the next connect).
    console: Option<u32>,
    /// The session whose injector runs, if one does.
    injector: Option<u32>,
}

impl Supervisor {
    pub fn new(console: Option<u32>) -> Self {
        Self {
            console,
            injector: None,
        }
    }

    /// The session at the console, as just read after a session change.
    pub fn console_changed(&mut self, console: Option<u32>) -> Option<Action> {
        self.console = console;
        match self.injector {
            Some(session) if Some(session) != console => {
                self.injector = None;
                Some(Action::Stop(session))
            }
            _ => None,
        }
    }

    /// A stream starts and needs an injector. `None` when the one it needs already
    /// runs, or when there is no console session to put it in.
    pub fn wanted(&mut self) -> Option<Action> {
        let console = self.console?;
        if self.injector == Some(console) {
            return None;
        }
        // An injector of another session would have been stopped when the console
        // left it; this is the first one, or the one after it ended.
        self.injector = Some(console);
        Some(Action::Launch(console))
    }

    /// The launch failed: no injector runs in that session.
    pub fn launch_failed(&mut self, session: u32) {
        if self.injector == Some(session) {
            self.injector = None;
        }
    }

    /// The injector of `session` ended, by itself or stopped. An exit reported late,
    /// for a session whose injector was already replaced, changes nothing.
    pub fn exited(&mut self, session: u32) {
        if self.injector == Some(session) {
            self.injector = None;
        }
    }

    /// The session whose injector runs.
    pub fn injector(&self) -> Option<u32> {
        self.injector
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stream_gets_an_injector_in_the_console_session_once() {
        let mut supervisor = Supervisor::new(Some(1));
        assert_eq!(supervisor.wanted(), Some(Action::Launch(1)));
        assert_eq!(supervisor.wanted(), None);
        assert_eq!(supervisor.injector(), Some(1));
    }

    #[test]
    fn no_console_session_means_no_injector() {
        let mut supervisor = Supervisor::new(None);
        assert_eq!(supervisor.wanted(), None);
        assert_eq!(supervisor.injector(), None);
    }

    #[test]
    fn the_console_leaving_a_session_stops_its_injector() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        assert_eq!(supervisor.console_changed(Some(2)), Some(Action::Stop(1)));
        assert_eq!(supervisor.injector(), None);
        assert_eq!(supervisor.wanted(), Some(Action::Launch(2)));
    }

    #[test]
    fn a_disconnected_console_stops_the_injector_and_launches_none() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        assert_eq!(supervisor.console_changed(None), Some(Action::Stop(1)));
        assert_eq!(supervisor.wanted(), None);
    }

    #[test]
    fn a_session_change_that_keeps_the_console_changes_nothing() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        assert_eq!(supervisor.console_changed(Some(1)), None);
        assert_eq!(supervisor.injector(), Some(1));
    }

    #[test]
    fn an_injector_that_ended_is_launched_again_at_the_next_stream() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        supervisor.exited(1);
        assert_eq!(supervisor.wanted(), Some(Action::Launch(1)));
    }

    #[test]
    fn a_late_exit_of_a_replaced_injector_changes_nothing() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        supervisor.console_changed(Some(2));
        supervisor.wanted();
        supervisor.exited(1);
        assert_eq!(supervisor.injector(), Some(2));
    }

    #[test]
    fn a_failed_launch_is_tried_again_at_the_next_stream() {
        let mut supervisor = Supervisor::new(Some(1));
        supervisor.wanted();
        supervisor.launch_failed(1);
        assert_eq!(supervisor.wanted(), Some(Action::Launch(1)));
    }
}
