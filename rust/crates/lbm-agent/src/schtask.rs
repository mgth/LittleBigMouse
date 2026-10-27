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

use std::path::PathBuf;

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
}

#[cfg(windows)]
impl ScheduledTask {
    /// C# `IsScheduled`: the task exists.
    pub fn is_scheduled(&self) -> bool {
        schtasks(&["/Query", "/TN", &self.name()]).is_ok()
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
}
