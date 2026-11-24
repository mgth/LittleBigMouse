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
    /// Launch an injector into this session, under this launch number: its exit is
    /// told with it ([`Supervisor::exited`]).
    Launch { session: u32, launch: u64 },
    /// Stop the injector of this launch: close its input, which makes it release what
    /// is pressed and leave.
    Stop { launch: u64 },
}

/// The injector that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Running {
    session: u32,
    launch: u64,
}

/// The state the decisions are taken on.
#[derive(Debug, Default)]
pub struct Supervisor {
    /// The session at the console; `None` while the console is attached to none
    /// (between a disconnect and the next connect).
    console: Option<u32>,
    injector: Option<Running>,
    /// The number of the last launch: an exit is told by it, so that the late exit of
    /// an injector of the same session, replaced since, is not taken for the new one's.
    launches: u64,
}

impl Supervisor {
    pub fn new(console: Option<u32>) -> Self {
        Self {
            console,
            ..Self::default()
        }
    }

    /// The session at the console, as just read after a session change.
    pub fn console_changed(&mut self, console: Option<u32>) -> Option<Action> {
        self.console = console;
        match self.injector {
            Some(running) if Some(running.session) != console => {
                self.injector = None;
                Some(Action::Stop {
                    launch: running.launch,
                })
            }
            _ => None,
        }
    }

    /// A stream starts and needs an injector. `None` when the one it needs already
    /// runs, or when there is no console session to put it in.
    pub fn wanted(&mut self) -> Option<Action> {
        let console = self.console?;
        if self
            .injector
            .is_some_and(|running| running.session == console)
        {
            return None;
        }
        // An injector of another session would have been stopped when the console
        // left it; this is the first one, or the one after it ended.
        self.launches += 1;
        let launch = self.launches;
        self.injector = Some(Running {
            session: console,
            launch,
        });
        Some(Action::Launch {
            session: console,
            launch,
        })
    }

    /// The launch failed: no injector runs for it.
    pub fn launch_failed(&mut self, launch: u64) {
        self.exited(launch);
    }

    /// The injector of `launch` ended, by itself or stopped. `true` when it was the one
    /// running; an exit told late, for a launch replaced since, changes nothing.
    pub fn exited(&mut self, launch: u64) -> bool {
        let current = self
            .injector
            .is_some_and(|running| running.launch == launch);
        if current {
            self.injector = None;
        }
        current
    }

    /// The session whose injector runs.
    pub fn injector(&self) -> Option<u32> {
        self.injector.map(|running| running.session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launched(action: Option<Action>) -> u64 {
        match action {
            Some(Action::Launch { launch, .. }) => launch,
            other => panic!("not a launch: {other:?}"),
        }
    }

    #[test]
    fn a_stream_gets_an_injector_in_the_console_session_once() {
        let mut supervisor = Supervisor::new(Some(1));
        assert!(matches!(
            supervisor.wanted(),
            Some(Action::Launch { session: 1, .. })
        ));
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
        let first = launched(supervisor.wanted());
        assert_eq!(
            supervisor.console_changed(Some(2)),
            Some(Action::Stop { launch: first })
        );
        assert_eq!(supervisor.injector(), None);
        assert!(matches!(
            supervisor.wanted(),
            Some(Action::Launch { session: 2, .. })
        ));
    }

    #[test]
    fn a_disconnected_console_stops_the_injector_and_launches_none() {
        let mut supervisor = Supervisor::new(Some(1));
        let first = launched(supervisor.wanted());
        assert_eq!(
            supervisor.console_changed(None),
            Some(Action::Stop { launch: first })
        );
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
        let first = launched(supervisor.wanted());
        assert!(supervisor.exited(first));
        assert!(launched(supervisor.wanted()) > first);
    }

    #[test]
    fn a_late_exit_of_a_replaced_injector_changes_nothing() {
        let mut supervisor = Supervisor::new(Some(1));
        let first = launched(supervisor.wanted());
        supervisor.console_changed(Some(2));
        supervisor.wanted();
        assert!(!supervisor.exited(first));
        assert_eq!(supervisor.injector(), Some(2));
    }

    #[test]
    fn the_console_back_in_the_same_session_is_not_stopped_by_the_old_injectors_exit() {
        let mut supervisor = Supervisor::new(Some(1));
        let first = launched(supervisor.wanted());
        supervisor.console_changed(Some(2));
        supervisor.console_changed(Some(1));
        let second = launched(supervisor.wanted());
        // The first injector's exit arrives only now: same session, older launch.
        assert!(!supervisor.exited(first));
        assert_eq!(supervisor.injector(), Some(1));
        assert!(supervisor.exited(second));
    }

    #[test]
    fn a_failed_launch_is_tried_again_at_the_next_stream() {
        let mut supervisor = Supervisor::new(Some(1));
        let first = launched(supervisor.wanted());
        supervisor.launch_failed(first);
        assert!(matches!(
            supervisor.wanted(),
            Some(Action::Launch { session: 1, .. })
        ));
    }
}
