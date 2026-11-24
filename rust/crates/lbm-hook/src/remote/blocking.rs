//! Whether Windows lets what this hook injects reach the desktop, and when to say so.
//!
//! UIPI (User Interface Privilege Isolation): input a process injects with `SendInput`
//! is dropped when the window in the foreground belongs to a process of a higher
//! integrity level — an elevated program, Task Manager, or a UIAccess one such as the
//! on-screen keyboard (`osk.exe` runs at High from a Medium session). Everything is
//! dropped then, the pointer's motion included: the master's user sees the remote's
//! pointer freeze and theirs come back blind. `SendInput` does not say so: the
//! documentation is explicit that neither its count nor `GetLastError` reflects a UIPI
//! block. So the hook compares integrity levels itself, the foreground's against its
//! own, the way Windows does.
//!
//! The decision and the reporting live here, apart from the Windows calls, so that both
//! can be tested anywhere.

use lbm_ipc::peer::InjectionBlock;

/// `SECURITY_MANDATORY_HIGH_RID`.
pub const HIGH: u32 = 0x3000;
/// `SECURITY_MANDATORY_MEDIUM_RID`.
pub const MEDIUM: u32 = 0x2000;

/// A process's token, as far as UIPI goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Integrity {
    /// The mandatory label's RID: 0x2000 Medium, 0x3000 High, 0x4000 System.
    pub level: u32,
    /// The token's `TokenUIAccess`: such a process is let through UIPI whatever the
    /// target's level.
    pub ui_access: bool,
}

/// What is known of the window in the foreground.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Foreground {
    /// No window has the focus (a desktop switch under way, a locked session): there
    /// is nothing to say about it.
    Nobody,
    /// A window whose process could be opened but whose token could not be read: a
    /// process this one may not query, which in a desktop session means a more
    /// privileged one (SYSTEM, another administrator's elevated token).
    Unreadable { process: String },
    Known {
        process: String,
        integrity: Integrity,
    },
}

/// Whether input injected by a process of `ours` integrity is dropped while
/// `foreground` has the focus. `None` for "it goes through" and for "this hook cannot
/// tell" (its own token unread): a warning that may be wrong is worse than none, since
/// it ends the master's crossings (see the agent's `InjectionBlocked`).
pub fn blocked(ours: Option<Integrity>, foreground: &Foreground) -> Option<InjectionBlock> {
    let ours = ours?;
    if ours.ui_access {
        return None;
    }
    match foreground {
        Foreground::Nobody => None,
        Foreground::Unreadable { process } => Some(InjectionBlock::elevated(process.clone())),
        Foreground::Known { process, integrity } => {
            (integrity.level > ours.level).then(|| InjectionBlock::elevated(process.clone()))
        }
    }
}

/// The file name of a process's executable path, as the warning shows it.
pub fn file_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_owned()
}

/// What the agent was last told ([`lbm_ipc::protocol::Event::InjectionBlocked`]), and
/// what it is to be told next.
#[derive(Debug, Default)]
pub struct Reporter {
    /// `None` before anything was said: the agent assumes nothing is blocked.
    said: Option<InjectionBlock>,
}

impl Reporter {
    pub const fn new() -> Self {
        Self { said: None }
    }

    /// A driven session begins while `now` is what the foreground allows. A session that
    /// begins blocked is said so even if it was already said: the master brings its
    /// pointer home on hearing it (the session before may have ended that way), and must
    /// do it again for this crossing. One that begins unblocked is said only if blocked
    /// was the last word.
    pub fn begin(&mut self, now: Option<InjectionBlock>) -> Option<Option<InjectionBlock>> {
        if now.is_none() && self.said.is_none() {
            return None;
        }
        self.said.clone_from(&now);
        Some(now)
    }

    /// The foreground changed during a session: said only if that changes the verdict
    /// (another elevated window after the first is still the same block, but the name
    /// shown changes with it).
    pub fn changed(&mut self, now: Option<InjectionBlock>) -> Option<Option<InjectionBlock>> {
        if now == self.said {
            return None;
        }
        self.said.clone_from(&now);
        Some(now)
    }

    /// `SendInput` put fewer events into the system than it was handed: not a UIPI block
    /// (that one is silent) but something as final — a secure desktop (UAC, Ctrl+Alt+Del)
    /// or a desktop this session does not own. Said once, as a block of unknown cause, if
    /// nothing was said yet: no integrity difference was seen, so nothing may advise
    /// elevation. What the foreground says next corrects it.
    pub fn short(&mut self) -> Option<Option<InjectionBlock>> {
        if self.said.is_some() {
            return None;
        }
        self.changed(Some(InjectionBlock::unknown()))
    }

    /// What was last said.
    pub fn said(&self) -> Option<&InjectionBlock> {
        self.said.as_ref()
    }
}

/// The foreground as last judged and what was said of it: one per process, behind one
/// lock, so that what the event hook and the injected batches observe is judged in the
/// order it was read.
///
/// A WinEvent is delivered late, from a queue, naming the window that took the focus
/// then. Judging that window would let an elevated one that held the focus a moment be
/// said blocked after a batch already saw an ordinary one back in front, and the master
/// would end a crossing that works. So the event only says "look again": the caller reads
/// the foreground under the lock and passes what it reads now, as a batch does.
#[derive(Debug, Default)]
pub struct Watch {
    /// The foreground window last judged (its handle as a number); 0 for none yet.
    last: isize,
    reporter: Reporter,
}

impl Watch {
    pub const fn new() -> Self {
        Self {
            last: 0,
            reporter: Reporter::new(),
        }
    }

    /// A session begins with `window` in the foreground, judged by `judge`.
    pub fn begin(
        &mut self,
        window: isize,
        judge: impl FnOnce() -> Option<InjectionBlock>,
    ) -> Option<Option<InjectionBlock>> {
        self.last = window;
        self.reporter.begin(judge())
    }

    /// A batch is about to be injected with `window` in the foreground: judged only if it
    /// is not the window last judged.
    pub fn batch(
        &mut self,
        window: isize,
        judge: impl FnOnce() -> Option<InjectionBlock>,
    ) -> Option<Option<InjectionBlock>> {
        if window == self.last {
            return None;
        }
        self.last = window;
        self.reporter.changed(judge())
    }

    /// The event hook said the foreground changed; `window` is the foreground read now,
    /// not the one the event named. Judged even if it is the window last judged: the
    /// event may have been the only sign of a change and back.
    pub fn event(
        &mut self,
        window: isize,
        judge: impl FnOnce() -> Option<InjectionBlock>,
    ) -> Option<Option<InjectionBlock>> {
        self.last = window;
        self.reporter.changed(judge())
    }

    /// See [`Reporter::short`].
    pub fn short(&mut self) -> Option<Option<InjectionBlock>> {
        self.reporter.short()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEDIUM_HOOK: Option<Integrity> = Some(Integrity {
        level: MEDIUM,
        ui_access: false,
    });
    const HIGH_HOOK: Option<Integrity> = Some(Integrity {
        level: HIGH,
        ui_access: false,
    });

    fn window(process: &str, level: u32, ui_access: bool) -> Foreground {
        Foreground::Known {
            process: process.to_owned(),
            integrity: Integrity { level, ui_access },
        }
    }

    fn block(process: &str) -> Option<InjectionBlock> {
        Some(InjectionBlock::elevated(process))
    }

    #[test]
    fn a_higher_foreground_blocks_a_hook_that_is_not_elevated() {
        // The real case of 2026-11-24, as read on the vm124: osk.exe is High and
        // UIAccess, the hook Medium.
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("osk.exe", HIGH, true)),
            block("osk.exe")
        );
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("Taskmgr.exe", HIGH, false)),
            block("Taskmgr.exe")
        );
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("LogonUI.exe", 0x4000, false)),
            block("LogonUI.exe")
        );
    }

    #[test]
    fn the_same_or_a_lower_level_goes_through() {
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("explorer.exe", MEDIUM, false)),
            None
        );
        // UIAccess alone does not raise a window above the hook's level: only the
        // level counts.
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("magnify.exe", MEDIUM, true)),
            None
        );
        assert_eq!(
            blocked(MEDIUM_HOOK, &window("sandboxed.exe", 0x1000, false)),
            None
        );
    }

    #[test]
    fn an_elevated_hook_reaches_an_elevated_window() {
        // StartElevated: the agent, and the hook it starts, run at High.
        assert_eq!(blocked(HIGH_HOOK, &window("osk.exe", HIGH, true)), None);
        // But not a SYSTEM one.
        assert_eq!(
            blocked(HIGH_HOOK, &window("LogonUI.exe", 0x4000, false)),
            block("LogonUI.exe")
        );
    }

    #[test]
    fn a_ui_access_hook_is_never_blocked() {
        let ui_access = Some(Integrity {
            level: MEDIUM,
            ui_access: true,
        });
        assert_eq!(blocked(ui_access, &window("osk.exe", HIGH, true)), None);
        assert_eq!(
            blocked(
                ui_access,
                &Foreground::Unreadable {
                    process: "x.exe".into()
                }
            ),
            None
        );
    }

    #[test]
    fn what_cannot_be_read_is_not_guessed_about_ourselves_but_is_about_the_window() {
        // Its own token unread: the hook says nothing rather than end crossings on a
        // guess.
        assert_eq!(blocked(None, &window("osk.exe", HIGH, true)), None);
        // No foreground window at all: nothing to say.
        assert_eq!(blocked(MEDIUM_HOOK, &Foreground::Nobody), None);
        // A window whose token this hook may not read belongs to a more privileged
        // process.
        assert_eq!(
            blocked(
                MEDIUM_HOOK,
                &Foreground::Unreadable {
                    process: "consent.exe".into()
                }
            ),
            block("consent.exe")
        );
    }

    #[test]
    fn the_file_name_is_what_the_warning_shows() {
        assert_eq!(file_name(r"C:\Windows\System32\osk.exe"), "osk.exe");
        assert_eq!(file_name("osk.exe"), "osk.exe");
        assert_eq!(file_name(""), "");
    }

    #[test]
    fn changes_are_said_once_and_nothing_is_said_of_nothing() {
        let mut reporter = Reporter::default();
        assert_eq!(reporter.changed(None), None, "unblocked is the default");
        assert_eq!(reporter.changed(block("osk.exe")), Some(block("osk.exe")));
        assert_eq!(reporter.changed(block("osk.exe")), None);
        assert_eq!(
            reporter.changed(block("Taskmgr.exe")),
            Some(block("Taskmgr.exe")),
            "the name shown follows the window"
        );
        assert_eq!(reporter.changed(None), Some(None));
        assert_eq!(reporter.changed(None), None);
    }

    #[test]
    fn a_session_that_begins_blocked_says_it_again() {
        let mut reporter = Reporter::default();
        assert_eq!(reporter.begin(None), None, "nothing to undo");
        assert_eq!(reporter.begin(block("osk.exe")), Some(block("osk.exe")));
        // The master brought its pointer home; the user crosses again, the window
        // still in front: said again, so that this crossing comes home too.
        assert_eq!(reporter.begin(block("osk.exe")), Some(block("osk.exe")));
        // The window went while nobody drove this machine.
        assert_eq!(reporter.begin(None), Some(None));
        assert_eq!(reporter.begin(None), None);
    }

    #[test]
    fn a_short_send_is_a_block_of_unknown_cause_only_when_nothing_was_said() {
        let mut reporter = Reporter::default();
        // A secure desktop: no foreground to name, no integrity seen. Not to be shown as
        // an elevated window, nor to advise elevation.
        let unknown = Some(InjectionBlock::unknown());
        assert_eq!(reporter.short(), Some(unknown.clone()));
        assert_eq!(
            unknown.map(|b| b.cause),
            Some(lbm_ipc::peer::BlockCause::Unknown)
        );
        assert_eq!(reporter.short(), None, "once");
        assert_eq!(
            reporter.changed(None),
            Some(None),
            "the foreground corrects it"
        );
        reporter.changed(block("osk.exe"));
        assert_eq!(
            reporter.short(),
            None,
            "a block already said keeps its name and cause"
        );
        assert_eq!(reporter.said(), block("osk.exe").as_ref());
    }

    #[test]
    fn a_late_event_for_an_elevated_window_already_gone_says_nothing() {
        const ORDINARY: isize = 1;
        const ELEVATED: isize = 2;
        let judge = |window: isize| {
            move || {
                if window == ELEVATED {
                    block("osk.exe")
                } else {
                    None
                }
            }
        };
        let mut watch = Watch::new();
        assert_eq!(watch.begin(ORDINARY, judge(ORDINARY)), None);
        // osk.exe took the focus a moment and gave it back. A batch reads the ordinary
        // window again: nothing changed for it.
        assert_eq!(watch.batch(ORDINARY, judge(ORDINARY)), None);
        // The event for osk.exe arrives now, from its queue: the foreground is read again
        // and it is the ordinary window — no block is said, the crossing goes on.
        assert_eq!(watch.event(ORDINARY, judge(ORDINARY)), None);
        // A real change is said by whichever observes it first, once.
        assert_eq!(
            watch.event(ELEVATED, judge(ELEVATED)),
            Some(block("osk.exe"))
        );
        assert_eq!(watch.batch(ELEVATED, judge(ELEVATED)), None);
        assert_eq!(watch.batch(ORDINARY, judge(ORDINARY)), Some(None));
    }

    #[test]
    fn a_batch_judges_only_a_new_window() {
        let mut watch = Watch::new();
        let mut judged = 0;
        watch.begin(1, || None);
        watch.batch(1, || {
            judged += 1;
            None
        });
        assert_eq!(judged, 0, "the same window is not judged again");
        watch.batch(2, || {
            judged += 1;
            None
        });
        assert_eq!(judged, 1);
    }
}
