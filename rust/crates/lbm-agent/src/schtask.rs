//! Starting with the session under Windows: C#'s scheduled task (`AutostartExtensions`),
//! now launching the agent — and, as the plan's decision D6 says, relaunching it when it
//! fails.
//!
//! The task keeps the C# name (`LittleBigMouse_<DOMAIN>_<user>`), so a 5.x task is the
//! agent's: it counts as scheduled, and the next save re-registers it on the agent (the
//! plan: "la tâche 5.x qui lance `LittleBigMouse.Ui.Avalonia.exe` est migrée").
//!
//! What the task is, is one readable document: a definition in XML, handed to the Task
//! Scheduler in memory (`ITaskFolder::RegisterTask`). Not through `schtasks /Create /XML`,
//! which wants it in a file: the only folder at hand is `%TEMP%`, which every unelevated
//! program of the user can write, and an elevated agent would register whatever had been
//! swapped in between the write and the read. Its settings are C#'s — a logon trigger for
//! this user, the requested run level (an elevated task, when refused, becomes a plain one
//! rather than none), no battery condition, no time limit, not in a remote session — plus a
//! restart on failure.
//!
//! An elevated task is registered only for an agent nobody else can replace
//! ([`crate::admin_only`]): it runs at every logon with full rights and no prompt.
//!
//! The agent is the task's only writer. The window (C#) used to register it too, on its
//! own executable, from its local save: it no longer touches it, and shows what the agent
//! says.
//!
//! The task is the account's, not an install's: one registered by a copy since moved or
//! uninstalled stays, pointing at a program that is gone, and every logon fails with
//! `ERROR_FILE_NOT_FOUND` (0x80070002) while the option reads on. Asking whether it is
//! scheduled therefore reads what it launches, and such a task is pointed back at this
//! agent, at its former run level and through the same guard as any registration — or
//! answered "not scheduled" when it cannot be (C#: mgth/LittleBigMouse#739, on 5.x).

use std::path::{Path, PathBuf};

/// C#'s task description.
const DESCRIPTION: &str = "Multi-dpi aware monitors mouse crossover";

/// The session's scheduled task, for one program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduledTask {
    /// The account, as Windows names it (`DOMAIN\user`).
    user: String,
    program: PathBuf,
}

impl ScheduledTask {
    pub fn new(user: impl Into<String>, program: PathBuf) -> Self {
        ScheduledTask {
            user: user.into(),
            program,
        }
    }

    /// This session's account, launching this executable.
    pub fn for_session() -> Option<Self> {
        let var = |name| std::env::var(name).ok().filter(|v| !v.is_empty());
        let user = format!("{}\\{}", var("USERDOMAIN")?, var("USERNAME")?);
        Some(ScheduledTask::new(user, std::env::current_exe().ok()?))
    }

    /// C# `ServiceName`: `LittleBigMouse_` and the account, its `\` made `_`.
    pub fn name(&self) -> String {
        format!("LittleBigMouse_{}", self.user.replace('\\', "_"))
    }

    /// The task definition; `elevated`: run with the highest privileges available (C#:
    /// `TaskRunLevel.Highest`, else `LUA`).
    pub fn xml(&self, elevated: bool) -> String {
        let user = escape(&self.user);
        let program = self.program.to_string_lossy();
        let command = escape(&program);
        // Its directory, split as Windows splits it (the tests run elsewhere too).
        let directory = escape(program.rfind(['\\', '/']).map_or("", |i| &program[..i]));
        let run_level = if elevated {
            "HighestAvailable"
        } else {
            "LeastPrivilege"
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.3" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>{DESCRIPTION}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>{run_level}</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <DisallowStartOnRemoteAppSession>true</DisallowStartOnRemoteAppSession>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT1M</Interval>
      <Count>3</Count>
    </RestartOnFailure>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <WorkingDirectory>{directory}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
        )
    }

    /// Whether the session starts something, from the task `found` under this name: a
    /// task that can only fail is handed to `retarget` (with its run level, `true` for
    /// an elevated one) to launch this program instead, and counts as scheduled only if
    /// that worked. `gone`: see [`Registered::launches`].
    #[cfg(any(windows, test))]
    fn scheduled_from(
        &self,
        found: Option<Registered>,
        gone: impl Fn(&Path) -> bool,
        retarget: impl FnOnce(bool) -> std::io::Result<()>,
    ) -> bool {
        let Some(task) = found else {
            return false;
        };
        if task.launches(gone) {
            return true;
        }
        eprintln!(
            "[lbm-agent] the logon task launches {:?}, which is gone: pointing it at {}",
            task.programs,
            self.program.display()
        );
        match retarget(task.elevated) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("[lbm-agent] cannot retarget the logon task: {error}");
                false
            }
        }
    }
}

/// A registered task, as read back from the Task Scheduler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registered {
    /// What each of its `Exec` actions launches, as stored (possibly quoted).
    pub programs: Vec<String>,
    /// Registered to run with the highest privileges available.
    pub elevated: bool,
}

impl Registered {
    /// Whether a logon can start anything: it launches some program, and none of them is
    /// known to be gone. `gone` answers for one absolute path; a path it cannot judge — a
    /// relative one, or one holding an environment variable the Task Scheduler expands —
    /// counts as there: a task is rewritten on proof that it can only fail, not on doubt.
    pub fn launches(&self, gone: impl Fn(&Path) -> bool) -> bool {
        !self.programs.is_empty() && !self.programs.iter().any(|p| program_gone(p, &gone))
    }
}

fn program_gone(program: &str, gone: &impl Fn(&Path) -> bool) -> bool {
    let path = program.trim().trim_matches('"');
    is_windows_absolute(path) && !path.contains('%') && gone(Path::new(path))
}

/// `C:\…` or `\\server\…`, read as Windows reads it (the tests run elsewhere too).
fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with(r"\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

/// `path` is known not to exist: looking it up says so, rather than failing for another
/// reason (a folder this account may not read).
pub fn is_gone(path: &Path) -> bool {
    matches!(std::fs::metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

#[cfg(windows)]
impl ScheduledTask {
    /// C# `IsScheduled`, made to mean what logons will do: the task exists and launches a
    /// program that is there. One whose program is gone is pointed back at this agent;
    /// one that cannot be read is taken as it is.
    pub fn is_scheduled(&self) -> bool {
        match self.registered() {
            Ok(found) => self.scheduled_from(found, is_gone, |elevated| self.set(true, elevated)),
            Err(error) => {
                eprintln!("[lbm-agent] the logon task cannot be read, left as it is: {error}");
                schtasks(&["/Query", "/TN", &self.name()]).is_ok()
            }
        }
    }

    /// The task registered under this name, read back: `None` when there is none.
    fn registered(&self) -> std::io::Result<Option<Registered>> {
        use windows::core::{Interface, BSTR, VARIANT};
        use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
        use windows::Win32::System::TaskScheduler::{
            IExecAction, ITaskService, TaskScheduler, TASK_ACTION_EXEC, TASK_ACTION_TYPE,
            TASK_RUNLEVEL_HIGHEST, TASK_RUNLEVEL_TYPE,
        };

        let _com = Com::enter()?;
        let none = VARIANT::default();
        // SAFETY: COM calls on interfaces this function owns, COM entered above; every out
        // pointer is a local that outlives its call.
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            service.Connect(&none, &none, &none, &none)?;
            let folder = service.GetFolder(&BSTR::from("\\"))?;
            let task = match folder.GetTask(&BSTR::from(self.name())) {
                Ok(task) => task,
                Err(error)
                    if error.code() == ERROR_FILE_NOT_FOUND.to_hresult()
                        || error.code() == ERROR_PATH_NOT_FOUND.to_hresult() =>
                {
                    return Ok(None)
                }
                Err(error) => return Err(error.into()),
            };
            let definition = task.Definition()?;

            let actions = definition.Actions()?;
            let mut count = 0;
            actions.Count(&mut count)?;
            let mut programs = Vec::new();
            // The Task Scheduler's collections count from 1.
            for index in 1..=count {
                let action = actions.get_Item(index)?;
                let mut kind = TASK_ACTION_TYPE::default();
                action.Type(&mut kind)?;
                if kind == TASK_ACTION_EXEC {
                    let mut path = BSTR::new();
                    action.cast::<IExecAction>()?.Path(&mut path)?;
                    programs.push(path.to_string());
                }
            }

            let mut run_level = TASK_RUNLEVEL_TYPE::default();
            definition.Principal()?.RunLevel(&mut run_level)?;
            Ok(Some(Registered {
                programs,
                elevated: run_level == TASK_RUNLEVEL_HIGHEST,
            }))
        }
    }

    /// C# `UpdateSchedule`: registered (replacing any older one, a 5.x task included) or
    /// removed. An elevated task the account may not register becomes a plain one.
    pub fn set(&self, enabled: bool, elevated: bool) -> std::io::Result<()> {
        let name = self.name();
        if !enabled {
            return match schtasks(&["/Query", "/TN", &name]) {
                Ok(()) => schtasks(&["/Delete", "/TN", &name, "/F"]),
                Err(_) => Ok(()),
            };
        }
        let elevated = elevated && self.may_run_elevated();
        match self.register(&name, elevated) {
            Err(_) if elevated => self.register(&name, false),
            result => result,
        }
    }

    /// From a folder the user can write to, an elevated task is administrator rights for
    /// whatever replaces the agent. Such a copy gets a plain task; the `StartElevated`
    /// option still asks UAC at startup ([`crate::elevation`]), where the user sees the
    /// request.
    fn may_run_elevated(&self) -> bool {
        let admin_only = crate::admin_only::is_admin_only(&self.program);
        if !admin_only {
            eprintln!(
                "[lbm-agent] not registering an elevated logon task: {} can be replaced \
                 without administrator rights",
                self.program.display()
            );
        }
        admin_only
    }

    /// Registers (or replaces, a 5.x task included) the definition, in memory.
    fn register(&self, name: &str, elevated: bool) -> std::io::Result<()> {
        use windows::core::{BSTR, VARIANT};
        use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
        use windows::Win32::System::TaskScheduler::{
            ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
        };

        let _com = Com::enter()?;
        let none = VARIANT::default();
        // SAFETY: COM calls on interfaces this function owns, COM entered above.
        unsafe {
            let service: ITaskService =
                CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)?;
            service.Connect(&none, &none, &none, &none)?;
            service.GetFolder(&BSTR::from("\\"))?.RegisterTask(
                &BSTR::from(name),
                &BSTR::from(self.xml(elevated)),
                TASK_CREATE_OR_UPDATE.0,
                &none,
                &none,
                TASK_LOGON_INTERACTIVE_TOKEN,
                &none,
            )?;
        }
        Ok(())
    }
}

/// COM on this thread for as long as it lives: entered multithreaded, or found already
/// entered in another mode (a window thread), which serves as well and is not ours to leave.
#[cfg(windows)]
struct Com(bool);

#[cfg(windows)]
impl Com {
    fn enter() -> std::io::Result<Self> {
        use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
        // SAFETY: plain COM initialisation of the calling thread.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result == RPC_E_CHANGED_MODE {
            return Ok(Com(false));
        }
        result.ok()?;
        Ok(Com(true))
    }
}

#[cfg(windows)]
impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful CoInitializeEx of `enter`, on the same thread.
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

/// Runs `schtasks.exe`: whether it succeeded, with what it said when it did not.
#[cfg(windows)]
fn schtasks(args: &[&str]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("schtasks.exe")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

/// XML text: `&`, `<`, `>` escaped (paths and account names hold no quote that matters in
/// element content).
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> ScheduledTask {
        ScheduledTask::new(
            r"DESKTOP-1\Mathieu",
            PathBuf::from(r"C:\Program Files\LittleBigMouse\lbm-agent.exe"),
        )
    }

    #[test]
    fn the_task_keeps_the_csharp_name() {
        assert_eq!(task().name(), "LittleBigMouse_DESKTOP-1_Mathieu");
    }

    #[test]
    fn the_definition_launches_the_agent_at_logon_and_again_when_it_fails() {
        let xml = task().xml(false);
        assert!(xml.contains("<Command>C:\\Program Files\\LittleBigMouse\\lbm-agent.exe</Command>"));
        assert!(
            xml.contains("<WorkingDirectory>C:\\Program Files\\LittleBigMouse</WorkingDirectory>")
        );
        assert!(xml.contains("<LogonTrigger>\n      <Enabled>true</Enabled>\n      <UserId>DESKTOP-1\\Mathieu</UserId>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<RestartOnFailure>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(task()
            .xml(true)
            .contains("<RunLevel>HighestAvailable</RunLevel>"));
    }

    #[test]
    fn names_are_escaped_in_the_definition() {
        let odd = ScheduledTask::new(r"CORP\R&D", PathBuf::from(r"C:\A<B>\lbm-agent.exe"));
        let xml = odd.xml(false);
        assert!(xml.contains("<UserId>CORP\\R&amp;D</UserId>"));
        assert!(xml.contains("<Command>C:\\A&lt;B&gt;\\lbm-agent.exe</Command>"));
    }

    fn registered(programs: &[&str], elevated: bool) -> Registered {
        Registered {
            programs: programs.iter().map(|p| p.to_string()).collect(),
            elevated,
        }
    }

    /// Only the old install is gone.
    fn old_install_gone(path: &Path) -> bool {
        under(path, r"D:\Old\")
    }

    /// A prefix, read as text: these are Windows paths, and the tests run elsewhere too.
    fn under(path: &Path, prefix: &str) -> bool {
        path.to_string_lossy().starts_with(prefix)
    }

    #[test]
    fn a_task_whose_program_is_there_launches() {
        let task = registered(&[r"C:\Program Files\LittleBigMouse\lbm-agent.exe"], false);
        assert!(task.launches(old_install_gone));
        // Quoted, as a path with spaces may be stored.
        let quoted = registered(
            &[r#""C:\Program Files\LittleBigMouse\lbm-agent.exe""#],
            false,
        );
        assert!(quoted.launches(|p| !under(p, r"C:\Program Files\")));
    }

    #[test]
    fn a_task_whose_program_is_gone_does_not_launch() {
        assert!(!registered(&[r"D:\Old\lbm-agent.exe"], false).launches(old_install_gone));
        assert!(!registered(&[r#""D:\Old\lbm-agent.exe""#], false).launches(old_install_gone));
        // One gone among several is a logon that fails.
        let both = registered(&[r"C:\Keep\a.exe", r"D:\Old\b.exe"], false);
        assert!(!both.launches(old_install_gone));
        // A share is a path like any other.
        assert!(!registered(&[r"\\server\share\lbm-agent.exe"], false).launches(|_| true));
        // Nothing to launch is nothing started.
        assert!(!registered(&[], false).launches(|_| false));
    }

    #[test]
    fn a_path_that_cannot_be_judged_counts_as_there() {
        let everything_gone = |_: &Path| true;
        // Expanded by the Task Scheduler, not here.
        assert!(
            registered(&[r"%ProgramFiles%\LittleBigMouse\lbm-agent.exe"], false)
                .launches(everything_gone)
        );
        // Looked up on the PATH.
        assert!(registered(&["lbm-agent.exe"], false).launches(everything_gone));
    }

    #[test]
    fn no_task_is_not_scheduled_and_nothing_is_registered() {
        let scheduled = task().scheduled_from(None, |_| true, |_| panic!("registered"));
        assert!(!scheduled);
    }

    #[test]
    fn a_task_that_launches_is_scheduled_and_left_alone() {
        // Another install still in place (a 5.x one included) is not this agent's to move.
        let found = registered(&[r"C:\Elsewhere\LittleBigMouse.Ui.Avalonia.exe"], true);
        let scheduled = task().scheduled_from(Some(found), |_| false, |_| panic!("registered"));
        assert!(scheduled);
    }

    #[test]
    fn a_task_whose_program_is_gone_is_retargeted_at_its_former_run_level() {
        for elevated in [false, true] {
            let mut asked = None;
            let found = registered(&[r"D:\Old\lbm-agent.exe"], elevated);
            let scheduled = task().scheduled_from(Some(found), old_install_gone, |e| {
                asked = Some(e);
                Ok(())
            });
            assert!(scheduled);
            assert_eq!(asked, Some(elevated));
        }
    }

    #[test]
    fn a_task_that_cannot_be_retargeted_is_not_scheduled() {
        // An elevated task, seen from an agent that may not replace it: every logon
        // would fail, so the option must not read on.
        let found = registered(&[r"D:\Old\lbm-agent.exe"], true);
        let scheduled = task().scheduled_from(Some(found), old_install_gone, |_| {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        });
        assert!(!scheduled);
    }

    #[test]
    fn only_a_missing_path_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_gone(dir.path()));
        assert!(is_gone(&dir.path().join("lbm-agent.exe")));
    }
}
