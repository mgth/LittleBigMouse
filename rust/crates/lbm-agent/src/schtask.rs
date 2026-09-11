//! Starting with the session under Windows: C#'s scheduled task (`AutostartExtensions`),
//! now launching the agent — and, as the plan's decision D6 says, relaunching it when it
//! fails.
//!
//! The task keeps the C# name (`LittleBigMouse_<DOMAIN>_<user>`), so a 5.x task is the
//! agent's: it counts as scheduled, and the next save re-registers it on the agent (the
//! plan: "la tâche 5.x qui lance `LittleBigMouse.Ui.Avalonia.exe` est migrée").
//!
//! It is registered through `schtasks.exe` from a task definition in XML rather than the
//! Task Scheduler's COM interfaces: what the task is, is then one readable document. Its
//! settings are C#'s — a logon trigger for this user, the requested run level (an elevated
//! task, when refused, becomes a plain one rather than none), no battery condition, no
//! time limit, not in a remote session — plus a restart on failure.

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
        match self.register(&name, elevated) {
            Err(_) if elevated => self.register(&name, false),
            result => result,
        }
    }

    fn register(&self, name: &str, elevated: bool) -> std::io::Result<()> {
        // schtasks reads the definition as the UTF-16 document it says it is.
        let file = std::env::temp_dir().join(format!("{name}-{}.xml", std::process::id()));
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(self.xml(elevated).encode_utf16().flat_map(u16::to_le_bytes));
        std::fs::write(&file, bytes)?;
        let result = schtasks(&[
            "/Create",
            "/TN",
            name,
            "/XML",
            &file.to_string_lossy(),
            "/F",
        ]);
        let _ = std::fs::remove_file(&file);
        result
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
