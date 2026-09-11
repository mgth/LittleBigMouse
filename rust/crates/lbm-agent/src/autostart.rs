//! Starting with the session under Linux (plan, phase 3: "autostart XDG sous Linux, qui
//! n'existe pas aujourd'hui"): an XDG autostart entry launching the agent. On Windows,
//! C#'s scheduled task (`AutostartExtensions`) moves over with the Windows agent.
//!
//! The entry follows the XDG autostart specification. The user's entry, in
//! `$XDG_CONFIG_HOME/autostart`, decides when it exists — disabled by `Hidden=true` (what
//! desktop settings write) or `X-GNOME-Autostart-enabled=false` — and otherwise an entry
//! of the same name in a system directory (`$XDG_CONFIG_DIRS/autostart`, one a package
//! may install) does. Turning autostart off removes the user's entry, or shadows a system
//! one with `Hidden=true`.

use std::io;
use std::path::{Path, PathBuf};

/// The entry's file name, the same in every directory.
pub const ENTRY: &str = "littlebigmouse-agent.desktop";

/// The session's XDG autostart, for one program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XdgAutostart {
    /// The user's autostart directory.
    user: PathBuf,
    /// The system ones, most important first.
    system: Vec<PathBuf>,
    /// What the entry launches.
    program: PathBuf,
}

impl XdgAutostart {
    pub fn new(user: PathBuf, system: Vec<PathBuf>, program: PathBuf) -> Self {
        XdgAutostart {
            user,
            system,
            program,
        }
    }

    /// This session's directories, launching this executable.
    pub fn for_session() -> Option<Self> {
        let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
        let config = var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|home| Path::new(&home).join(".config")))?;
        let system = var("XDG_CONFIG_DIRS")
            .map(|dirs| std::env::split_paths(&dirs).collect())
            .unwrap_or_else(|| vec![PathBuf::from("/etc/xdg")]);
        Some(XdgAutostart::new(
            config.join("autostart"),
            system.into_iter().map(|d| d.join("autostart")).collect(),
            std::env::current_exe().ok()?,
        ))
    }

    /// C# `IsAutostartScheduled`: whether the session starts the agent.
    pub fn is_scheduled(&self) -> bool {
        match std::fs::read_to_string(self.user.join(ENTRY)) {
            Ok(entry) => enabled(&entry),
            Err(_) => self.system_entry().is_some_and(|entry| enabled(&entry)),
        }
    }

    /// C# `SetAutostart`: aligns the session's autostart on `enabled`.
    pub fn set(&self, enabled: bool) -> io::Result<()> {
        let user = self.user.join(ENTRY);
        if enabled {
            return write(&user, &self.entry(false));
        }
        if self.system_entry().is_some() {
            // Removing ours would let the system's start the agent: shadow it instead.
            return write(&user, &self.entry(true));
        }
        match std::fs::remove_file(&user) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }

    fn system_entry(&self) -> Option<String> {
        self.system
            .iter()
            .find_map(|dir| std::fs::read_to_string(dir.join(ENTRY)).ok())
    }

    /// The entry; `hidden`: the one that turns a system entry off.
    fn entry(&self, hidden: bool) -> String {
        format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=LittleBigMouse\n\
             Comment=Mouse crossing between monitors of different sizes and resolutions\n\
             Exec={}\n\
             Terminal=false\n\
             NoDisplay=true\n\
             X-GNOME-Autostart-enabled={}\n\
             {}",
            exec_quote(&self.program),
            !hidden,
            if hidden { "Hidden=true\n" } else { "" }
        )
    }
}

/// Whether an entry is on: neither `Hidden=true` nor GNOME's switch off.
fn enabled(entry: &str) -> bool {
    !entry.lines().map(str::trim).any(|line| {
        line.eq_ignore_ascii_case("Hidden=true")
            || line.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
    })
}

/// The program as an `Exec` argument (desktop entry specification): quoted when it holds
/// a reserved character, `"`, `` ` ``, `$` and `\` escaped inside the quotes, `%` doubled.
fn exec_quote(program: &Path) -> String {
    let program = program.to_string_lossy().replace('%', "%%");
    const RESERVED: &[char] = &[
        ' ', '\t', '\n', '"', '\'', '\\', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(',
        ')', '`',
    ];
    if !program.contains(RESERVED) {
        return program;
    }
    let mut quoted = String::from('"');
    for c in program.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

/// Atomically: a torn entry would be a session that starts nothing, or garbage.
fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let staged = path.with_extension("desktop.tmp");
    std::fs::write(&staged, text)?;
    std::fs::rename(&staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn autostart(root: &Path) -> XdgAutostart {
        XdgAutostart::new(
            root.join("user/autostart"),
            vec![root.join("system/autostart")],
            PathBuf::from("/usr/bin/lbm-agent"),
        )
    }

    #[test]
    fn turned_on_and_off_in_the_users_directory() {
        let root = tempfile::tempdir().unwrap();
        let autostart = autostart(root.path());
        assert!(!autostart.is_scheduled());

        autostart.set(true).unwrap();
        assert!(autostart.is_scheduled());
        let entry =
            std::fs::read_to_string(root.path().join("user/autostart").join(ENTRY)).unwrap();
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("\nExec=/usr/bin/lbm-agent\n"));
        assert!(!entry.contains("Hidden"));

        autostart.set(false).unwrap();
        assert!(!autostart.is_scheduled());
        assert!(!root.path().join("user/autostart").join(ENTRY).exists());
        autostart.set(false).unwrap();
    }

    #[test]
    fn a_system_entry_is_shadowed_not_left_to_start_the_agent() {
        let root = tempfile::tempdir().unwrap();
        let autostart = autostart(root.path());
        write(
            &root.path().join("system/autostart").join(ENTRY),
            &autostart.entry(false),
        )
        .unwrap();
        assert!(autostart.is_scheduled(), "the package's entry starts it");

        autostart.set(false).unwrap();
        assert!(!autostart.is_scheduled());
        let user = std::fs::read_to_string(root.path().join("user/autostart").join(ENTRY)).unwrap();
        assert!(user.contains("\nHidden=true\n"));

        autostart.set(true).unwrap();
        assert!(autostart.is_scheduled());
    }

    #[test]
    fn an_entry_the_desktop_turned_off_is_off() {
        let root = tempfile::tempdir().unwrap();
        let autostart = autostart(root.path());
        autostart.set(true).unwrap();
        let path = root.path().join("user/autostart").join(ENTRY);
        let entry = std::fs::read_to_string(&path).unwrap();
        std::fs::write(
            &path,
            entry.replace(
                "X-GNOME-Autostart-enabled=true",
                "X-GNOME-Autostart-enabled=false",
            ),
        )
        .unwrap();
        assert!(!autostart.is_scheduled());
        std::fs::write(&path, format!("{entry}Hidden=true\n")).unwrap();
        assert!(!autostart.is_scheduled());
    }

    #[test]
    fn a_program_path_is_quoted_as_the_specification_says() {
        assert_eq!(
            exec_quote(Path::new("/usr/bin/lbm-agent")),
            "/usr/bin/lbm-agent"
        );
        assert_eq!(
            exec_quote(Path::new("/home/me/My Apps/lbm-agent")),
            r#""/home/me/My Apps/lbm-agent""#
        );
        assert_eq!(
            exec_quote(Path::new("/opt/$lbm/100%/a\"b")),
            r#""/opt/\$lbm/100%%/a\"b""#
        );
    }
}
