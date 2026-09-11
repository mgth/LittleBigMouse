//! Opportunistic startup elevation under Windows — port of C#'s `WindowsElevation`
//! (#512, #400).
//!
//! The agent starts for everyone: its manifest asks for nothing (a `requireAdministrator`
//! one meant, for a standard user, a UAC credential prompt nobody could answer and an
//! application that never launched). Elevation is taken only when it is both **wanted**
//! (the `StartElevated` option) and **possible** (a split-token administrator): one UAC
//! consent, through a relaunch of this executable.
//!
//! A standard user simply runs unelevated. What that costs is UIPI: the hook cannot inject
//! input while an elevated window holds the focus — and the hook a relaunched agent
//! launches inherits its elevation.
//!
//! The decision is taken **before the instance lock**: the relaunched agent takes it the
//! moment this one returns.

use std::ffi::OsStr;

/// C#'s `TOKEN_ELEVATION_TYPE`: no split token (a standard user, or UAC off), an elevated
/// token, or the filtered token of an administrator — the only one UAC can elevate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    Default,
    Full,
    Limited,
}

/// C# `ShouldRelaunchElevated`, as a decision on what was read.
pub fn should_relaunch(elevation: Elevation, requested: bool) -> bool {
    requested && elevation == Elevation::Limited
}

/// The arguments of a relaunch: this agent's own, plus every `LBM_*` variable of this
/// environment as `--env:NAME=value`.
///
/// `ShellExecute("runas")` gives the elevated child a fresh environment built from the
/// user profile, so the diagnostic variables (#506) set in the launching shell would be
/// lost on the way. C# forwards them the same way, and applies them before anything reads
/// the environment.
pub fn relaunch_arguments(
    args: impl IntoIterator<Item = String>,
    environment: impl IntoIterator<Item = (String, String)>,
) -> Vec<String> {
    let mut forwarded: Vec<String> = args.into_iter().collect();
    let mut variables: Vec<String> = environment
        .into_iter()
        .filter(|(name, _)| name.to_uppercase().starts_with("LBM_"))
        .map(|(name, value)| format!("--env:{name}={value}"))
        .collect();
    variables.sort();
    forwarded.extend(variables);
    forwarded
}

/// The `--env:NAME=value` arguments applied to this environment, and the rest given back
/// (C#: `Program` applies and strips them before anything reads the environment).
pub fn apply_environment_arguments(args: Vec<String>) -> Vec<String> {
    args.into_iter()
        .filter(|arg| match arg.strip_prefix("--env:") {
            Some(setting) => {
                if let Some((name, value)) = setting.split_once('=') {
                    // SAFETY (Rust 2024): before any thread of this agent is started.
                    unsafe { std::env::set_var(name, value) };
                }
                false
            }
            None => true,
        })
        .collect()
}

/// One command line out of arguments, quoted as `CommandLineToArgvW` reads them back
/// (`ShellExecuteW` takes a line, not a list).
pub fn command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut quoted = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                // The backslashes before a quote, and the quote itself, are escaped.
                quoted.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                backslashes = 0;
            }
            _ => backslashes = 0,
        }
        if c != '"' {
            quoted.push(c);
        } else {
            quoted.push('"');
        }
    }
    // The backslashes before the closing quote are escaped too.
    quoted.extend(std::iter::repeat_n('\\', backslashes));
    quoted.push('"');
    quoted
}

#[cfg(windows)]
mod win {
    use super::*;

    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{GetTokenInformation, TokenElevationType, TOKEN_QUERY};
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// This process's elevation (C#: `TokenElevationType`).
    pub fn elevation() -> Elevation {
        let mut token = HANDLE::default();
        // SAFETY: opens this process's own token for query.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
            return Elevation::Default;
        }
        let mut kind = 0u32;
        let mut length = 0;
        // SAFETY: the buffer is a u32 of the size given.
        let read = unsafe {
            GetTokenInformation(
                token,
                TokenElevationType,
                Some((&mut kind as *mut u32).cast()),
                std::mem::size_of::<u32>() as u32,
                &mut length,
            )
        };
        // SAFETY: the token opened above.
        unsafe {
            let _ = CloseHandle(token);
        }
        match (read.is_ok(), kind) {
            (true, 2) => Elevation::Full,
            (true, 3) => Elevation::Limited,
            _ => Elevation::Default,
        }
    }

    /// C# `RelaunchElevated`: this executable again, elevated, with `args`. `true` when
    /// the elevated agent is on its way — the caller must then leave **without** taking
    /// the instance lock. A refused UAC gives `false`: this agent runs on, unelevated.
    pub fn relaunch(args: &[String]) -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let directory = exe.parent().map(HSTRING::from).unwrap_or_default();
        let (exe, parameters) = (
            HSTRING::from(exe.as_path()),
            HSTRING::from(command_line(args)),
        );
        // SAFETY: the strings outlive the call; ShellExecuteW starts a process and
        // returns a value above 32 when it did.
        let started = unsafe {
            ShellExecuteW(
                None,
                &HSTRING::from("runas"),
                &exe,
                &parameters,
                &directory,
                SW_SHOWNORMAL,
            )
        };
        started.0 as isize > 32
    }
}

#[cfg(windows)]
pub use win::{elevation, relaunch};

/// The `StartElevated` option, as the store holds it — and, before the first launch has
/// imported it (D2), as the 5.x registry holds it.
pub fn start_elevated_requested<S: lbm_store::LayoutStore>(store: &S) -> bool {
    if let Some(stored) = store
        .read("", &[])
        .ok()
        .and_then(|data| data.global_options)
        .and_then(|options| options.start_elevated)
    {
        return stored;
    }
    registry_start_elevated()
}

#[cfg(windows)]
fn registry_start_elevated() -> bool {
    use lbm_store::registry_layout_store::try_get_bool;
    use lbm_store::windows_registry::WindowsKey;

    WindowsKey::open_current_user(r"SOFTWARE\Mgth\LittleBigMouse")
        .and_then(|root| try_get_bool(&root, "StartElevated"))
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn registry_start_elevated() -> bool {
    false
}

/// Whether `name` is one of the variables a relaunch forwards.
pub fn is_diagnostic_variable(name: &OsStr) -> bool {
    name.to_string_lossy().to_uppercase().starts_with("LBM_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elevation_is_taken_only_when_wanted_and_possible() {
        assert!(
            should_relaunch(Elevation::Limited, true),
            "a split-token admin"
        );
        assert!(
            !should_relaunch(Elevation::Default, true),
            "a standard user would face a credential prompt"
        );
        assert!(!should_relaunch(Elevation::Full, true), "already elevated");
        assert!(!should_relaunch(Elevation::Limited, false), "not asked for");
    }

    #[test]
    fn a_relaunch_carries_the_arguments_and_the_diagnostic_variables() {
        let args = ["--fake-hook".to_owned(), "--no-tray".to_owned()];
        let environment = [
            ("PATH".to_owned(), "/usr/bin".to_owned()),
            ("LBM_HOOK_ENDPOINT".to_owned(), r"\\.\pipe\x".to_owned()),
            ("lbm_trace".to_owned(), "1".to_owned()),
        ];
        assert_eq!(
            relaunch_arguments(args, environment),
            [
                "--fake-hook",
                "--no-tray",
                r"--env:LBM_HOOK_ENDPOINT=\\.\pipe\x",
                "--env:lbm_trace=1",
            ]
        );
    }

    #[test]
    fn the_environment_arguments_are_applied_and_stripped() {
        let rest = apply_environment_arguments(vec![
            "--fake-hook".to_owned(),
            "--env:LBM_TEST_ELEVATION=42".to_owned(),
            "--data-dir".to_owned(),
        ]);
        assert_eq!(rest, ["--fake-hook", "--data-dir"]);
        assert_eq!(std::env::var("LBM_TEST_ELEVATION").as_deref(), Ok("42"));
        assert!(is_diagnostic_variable(OsStr::new("LBM_HOOK_ENDPOINT")));
        assert!(!is_diagnostic_variable(OsStr::new("PATH")));
    }

    #[test]
    fn the_command_line_is_read_back_as_it_was_written() {
        assert_eq!(
            command_line(&["--hook".to_owned(), r"C:\Program Files\a.exe".to_owned()]),
            r#"--hook "C:\Program Files\a.exe""#
        );
        // A trailing backslash inside quotes, and a quote in an argument.
        assert_eq!(
            command_line(&[r"C:\dir with space\".to_owned()]),
            r#""C:\dir with space\\""#
        );
        assert_eq!(command_line(&[r#"a "b" c"#.to_owned()]), r#""a \"b\" c""#);
        assert_eq!(command_line(&[String::new()]), r#""""#);
    }
}
