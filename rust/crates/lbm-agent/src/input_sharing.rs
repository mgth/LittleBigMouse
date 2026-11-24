//! Finds the other mouse-and-keyboard sharing tools running on this machine
//! (`lbm_ipc::sharing`): Deskflow, Synergy, Barrier, Input Leap.
//!
//! **Looked at, never watched.** The runtime calls [`scan`] at the moments the answer
//! matters — the multi-PC mode turned on, a link with another machine made, the
//! engine started, a frontend subscribing — and keeps what it found in its state until
//! the next one. Nothing runs in between: a tool started or stopped afterwards is seen
//! at the next of those moments, not before (the project's rule: events only, nothing
//! active when the mode is off).
//!
//! The processes: `/proc` on Linux, a Toolhelp snapshot on Windows. Whether one holds
//! a connection, from the same single look at the TCP table: on Linux `/proc/net/tcp`
//! and `tcp6` matched to the process's own sockets (`/proc/<pid>/fd`), on Windows
//! `GetExtendedTcpTable`, which names each connection's process.

use lbm_ipc::sharing::{self, Role, Running};
#[cfg(any(target_os = "linux", all(test, unix)))]
use std::path::Path;

/// The sharing tools running now, by process id.
#[cfg(target_os = "linux")]
pub fn scan() -> Vec<Running> {
    scan_proc(Path::new("/proc"))
}

#[cfg(windows)]
pub fn scan() -> Vec<Running> {
    windows::scan()
}

#[cfg(not(any(target_os = "linux", windows)))]
pub fn scan() -> Vec<Running> {
    Vec::new()
}

/// Whether a new multi-PC state is a moment to look, and why: the mode turned on, or a
/// machine linked that was not. Each is `(mode on, machines linked)`, before and now.
pub fn moment(before: (bool, &[String]), now: (bool, &[String])) -> Option<&'static str> {
    if now.0 && !before.0 {
        Some("the multi-PC mode is on")
    } else if now.1.iter().any(|id| !before.1.contains(id)) {
        Some("a machine linked")
    } else {
        None
    }
}

/// One line of `/proc/net/tcp` or `tcp6`.
#[cfg(any(target_os = "linux", all(test, unix)))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Socket {
    local_port: u16,
    remote_port: u16,
    established: bool,
    inode: u64,
}

/// The sockets of a `/proc/net/tcp`-shaped text (the heading line, then one per
/// socket: `sl local rem st … uid timeout inode …`, ports in hexadecimal after the
/// last colon, state `01` for an established connection).
#[cfg(any(target_os = "linux", all(test, unix)))]
fn parse_tcp(text: &str) -> Vec<Socket> {
    let port = |address: &str| {
        address
            .rsplit_once(':')
            .and_then(|(_, port)| u16::from_str_radix(port, 16).ok())
    };
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            Some(Socket {
                local_port: port(fields.get(1)?)?,
                remote_port: port(fields.get(2)?)?,
                established: *fields.get(3)? == "01",
                inode: fields.get(9)?.parse().ok()?,
            })
        })
        .collect()
}

#[cfg(any(target_os = "linux", windows, test))]
/// The end a connection on the tools' default port says a process is: the one
/// listening there is a server, the one dialling it a client.
fn role_from_ports(local: u16, remote: u16) -> Option<Role> {
    if local == sharing::DEFAULT_PORT {
        Some(Role::Server)
    } else if remote == sharing::DEFAULT_PORT {
        Some(Role::Client)
    } else {
        None
    }
}

/// Under a `/proc`-like `root`: the sharing tools' processes, of any user (another
/// user's Deskflow moves the same pointer), in pid order.
#[cfg(any(target_os = "linux", all(test, unix)))]
pub fn scan_proc(root: &Path) -> Vec<Running> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut sockets = Vec::new();
    for table in ["net/tcp", "net/tcp6"] {
        if let Ok(text) = std::fs::read_to_string(root.join(table)) {
            sockets.extend(parse_tcp(&text));
        }
    }
    let established: Vec<Socket> = sockets.into_iter().filter(|s| s.established).collect();
    let mut found: Vec<Running> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let pid = entry.file_name().to_str()?.parse::<u32>().ok()?;
            let dir = entry.path();
            let comm = std::fs::read_to_string(dir.join("comm")).ok()?;
            let process = comm.trim_end_matches('\n').to_owned();
            let known = sharing::classify(&process)?;
            let cmdline = std::fs::read(dir.join("cmdline")).unwrap_or_default();
            let cmdline = String::from_utf8_lossy(&cmdline);
            // A window is no end of anything, whatever its arguments.
            let role = if known.window {
                None
            } else {
                known
                    .role
                    .or_else(|| sharing::role_from_args(cmdline.split('\0')))
            };
            // Its own sockets, when its descriptors may be read (this user's).
            let own: Option<Vec<Socket>> = std::fs::read_dir(dir.join("fd")).ok().map(|fds| {
                let inodes: Vec<u64> = fds
                    .filter_map(Result::ok)
                    .filter_map(|fd| std::fs::read_link(fd.path()).ok())
                    .filter_map(|target| {
                        target
                            .to_str()?
                            .strip_prefix("socket:[")?
                            .strip_suffix(']')?
                            .parse()
                            .ok()
                    })
                    .collect();
                established
                    .iter()
                    .filter(|s| inodes.contains(&s.inode))
                    .copied()
                    .collect()
            });
            let (role, connected) = match own {
                Some(own) => (
                    role.or_else(|| {
                        own.iter()
                            .find_map(|s| role_from_ports(s.local_port, s.remote_port))
                    }),
                    Some(!own.is_empty()),
                ),
                // Another user's: only a connection on the default port says anything,
                // and it is not known to be this process's.
                None => (
                    role,
                    established
                        .iter()
                        .any(|s| role_from_ports(s.local_port, s.remote_port).is_some())
                        .then_some(true),
                ),
            };
            Some(Running {
                tool: known.tool,
                process,
                pid,
                role,
                window: known.window,
                connected,
            })
        })
        .collect();
    found.sort_by_key(|r| r.pid);
    found
}

#[cfg(windows)]
mod windows {
    use super::role_from_ports;
    use lbm_ipc::sharing::{self, Running};
    use std::collections::HashMap;
    use windows::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID, MIB_TCP_STATE_ESTAB,
        TCP_TABLE_OWNER_PID_ALL,
    };
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    const AF_INET: u32 = 2;
    const AF_INET6: u32 = 23;

    /// A port as the table holds it: network order, in the low 16 bits.
    fn port(raw: u32) -> u16 {
        u16::from_be(raw as u16)
    }

    /// The table of one address family, in a buffer aligned for its rows; `None` when
    /// it could not be read.
    fn table(family: u32) -> Option<Vec<u32>> {
        let mut size = 0u32;
        for _ in 0..4 {
            let mut buffer = vec![0u32; (size as usize).div_ceil(4).max(1)];
            size = (buffer.len() * 4) as u32;
            // SAFETY: `buffer` holds `size` bytes, aligned for the table's u32 fields.
            let status = unsafe {
                GetExtendedTcpTable(
                    Some(buffer.as_mut_ptr().cast()),
                    &mut size,
                    false,
                    family,
                    TCP_TABLE_OWNER_PID_ALL,
                    0,
                )
            };
            if status == NO_ERROR.0 {
                return Some(buffer);
            }
            if status != ERROR_INSUFFICIENT_BUFFER.0 {
                return None;
            }
        }
        None
    }

    /// The established connections of each process: (local port, remote port).
    /// `None` when neither table could be read.
    fn established() -> Option<HashMap<u32, Vec<(u16, u16)>>> {
        let mut by_pid: HashMap<u32, Vec<(u16, u16)>> = HashMap::new();
        let mut read = false;
        let estab = MIB_TCP_STATE_ESTAB.0 as u32;
        if let Some(buffer) = table(AF_INET) {
            read = true;
            // SAFETY: a table GetExtendedTcpTable wrote: a count, then that many rows.
            let rows = unsafe {
                let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
                std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
            };
            for row in rows.iter().filter(|r| r.dwState == estab) {
                by_pid
                    .entry(row.dwOwningPid)
                    .or_default()
                    .push((port(row.dwLocalPort), port(row.dwRemotePort)));
            }
        }
        if let Some(buffer) = table(AF_INET6) {
            read = true;
            // SAFETY: as above, for the IPv6 table.
            let rows = unsafe {
                let table = &*(buffer.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID);
                std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize)
            };
            for row in rows.iter().filter(|r| r.dwState == estab) {
                by_pid
                    .entry(row.dwOwningPid)
                    .or_default()
                    .push((port(row.dwLocalPort), port(row.dwRemotePort)));
            }
        }
        read.then_some(by_pid)
    }

    /// Every session's: a Deskflow started by its service runs elevated, maybe in
    /// another, and still moves this pointer when it is the console's.
    pub fn scan() -> Vec<Running> {
        // SAFETY: a snapshot of the process list, closed below.
        let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
            return Vec::new();
        };
        let mut processes = Vec::new();
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        // SAFETY: the entry is sized as the API requires, the snapshot is open.
        let mut next = unsafe { Process32FirstW(snapshot, &mut entry) };
        while next.is_ok() {
            let length = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let exe = String::from_utf16_lossy(&entry.szExeFile[..length]);
            if let Some(known) = sharing::classify(&exe) {
                processes.push((entry.th32ProcessID, exe, known));
            }
            // SAFETY: as above.
            next = unsafe { Process32NextW(snapshot, &mut entry) };
        }
        // SAFETY: the snapshot opened above.
        unsafe {
            let _ = CloseHandle(snapshot);
        }
        if processes.is_empty() {
            return Vec::new();
        }
        let connections = established();
        let mut found: Vec<Running> = processes
            .into_iter()
            .map(|(pid, process, known)| {
                let own = connections
                    .as_ref()
                    .map(|c| c.get(&pid).cloned().unwrap_or_default());
                Running {
                    tool: known.tool,
                    process,
                    pid,
                    window: known.window,
                    // Windows does not hand out another process's command line simply:
                    // `deskflow-core` says its end through its connection, if it has one.
                    role: known.role.or_else(|| {
                        own.as_ref()?
                            .iter()
                            .find_map(|&(local, remote)| role_from_ports(local, remote))
                    }),
                    connected: own.map(|own| !own.is_empty()),
                }
            })
            .collect();
        found.sort_by_key(|r| r.pid);
        found
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use lbm_ipc::sharing::Tool;

    const TCP: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000:60E0 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 111 1 0000000000000000 100 0 0 10 0
   1: 0100A8C0:60E0 0200A8C0:D431 01 00000000:00000000 02:000A7B2E 00000000  1000        0 222 2 0000000000000000 20 4 30 10 -1
   2: 0100A8C0:C350 0300A8C0:60E0 01 00000000:00000000 02:000A7B2E 00000000  1000        0 333 2 0000000000000000 20 4 30 10 -1
";

    #[test]
    fn the_tcp_table_is_read_port_state_and_inode() {
        let sockets = parse_tcp(TCP);
        assert_eq!(
            sockets,
            [
                Socket {
                    local_port: 24800,
                    remote_port: 0,
                    established: false,
                    inode: 111
                },
                Socket {
                    local_port: 24800,
                    remote_port: 54321,
                    established: true,
                    inode: 222
                },
                Socket {
                    local_port: 50000,
                    remote_port: 24800,
                    established: true,
                    inode: 333
                },
            ]
        );
        assert!(parse_tcp("heading only\n").is_empty());
        assert!(parse_tcp("heading\n garbage line\n").is_empty());
    }

    fn process(root: &Path, pid: u32, comm: &str, args: &[&str], sockets: Option<&[u64]>) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("comm"), format!("{comm}\n")).unwrap();
        let mut cmdline = args.join("\0");
        cmdline.push('\0');
        std::fs::write(dir.join("cmdline"), cmdline).unwrap();
        if let Some(sockets) = sockets {
            let fd = dir.join("fd");
            std::fs::create_dir_all(&fd).unwrap();
            std::os::unix::fs::symlink("/dev/null", fd.join("0")).unwrap();
            for (n, inode) in sockets.iter().enumerate() {
                std::os::unix::fs::symlink(
                    format!("socket:[{inode}]"),
                    fd.join((n + 3).to_string()),
                )
                .unwrap();
            }
        }
    }

    fn proc_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("net")).unwrap();
        std::fs::write(root.path().join("net/tcp"), TCP).unwrap();
        std::fs::write(root.path().join("net/tcp6"), "heading\n").unwrap();
        root
    }

    #[test]
    fn nothing_running_is_nothing_found() {
        let root = proc_root();
        process(
            root.path(),
            10,
            "kwin_wayland",
            &["kwin_wayland"],
            Some(&[]),
        );
        process(root.path(), 11, "lbm-agent", &["lbm-agent"], Some(&[222]));
        assert!(scan_proc(root.path()).is_empty());
        assert!(scan_proc(&root.path().join("absent")).is_empty());
    }

    /// The case of 2026-11-24: a `deskflow-core server` left running without its
    /// window, still linked to a client. The window found too, connected to nothing.
    #[test]
    fn an_orphaned_server_holding_a_client_is_found_connected() {
        let root = proc_root();
        process(
            root.path(),
            40,
            "deskflow-core",
            &["/usr/bin/deskflow-core", "server", "-f"],
            Some(&[111, 222]),
        );
        process(
            root.path(),
            30,
            "deskflow",
            &["/usr/bin/deskflow"],
            Some(&[]),
        );
        assert_eq!(
            scan_proc(root.path()),
            [
                Running {
                    tool: Tool::Deskflow,
                    process: "deskflow".to_owned(),
                    pid: 30,
                    role: None,
                    window: true,
                    connected: Some(false),
                },
                Running {
                    tool: Tool::Deskflow,
                    process: "deskflow-core".to_owned(),
                    pid: 40,
                    role: Some(Role::Server),
                    window: false,
                    connected: Some(true),
                },
            ]
        );
    }

    #[test]
    fn a_listening_server_alone_is_not_connected() {
        let root = proc_root();
        process(
            root.path(),
            50,
            "barriers",
            &["barriers", "--no-daemon"],
            Some(&[111]),
        );
        let found = scan_proc(root.path());
        assert_eq!(found[0].role, Some(Role::Server));
        assert_eq!(found[0].connected, Some(false));
    }

    /// No word on the command line: the end is the one its connection to the default
    /// port says.
    #[test]
    fn a_core_without_its_word_takes_its_end_from_its_connection() {
        let root = proc_root();
        process(
            root.path(),
            60,
            "synergy-core",
            &["synergy-core", "--debug"],
            Some(&[333]),
        );
        let found = scan_proc(root.path());
        assert_eq!(found[0].tool, Tool::Synergy);
        assert_eq!(found[0].role, Some(Role::Client));
        assert_eq!(found[0].connected, Some(true));
    }

    /// Another user's: its sockets cannot be read. A connection on the default port
    /// says "connected"; none says nothing, since it may use another port.
    #[test]
    fn another_users_process_is_connected_only_by_the_default_port() {
        let root = proc_root();
        process(root.path(), 70, "input-leapc", &["input-leapc"], None);
        let found = scan_proc(root.path());
        assert_eq!(found[0].role, Some(Role::Client));
        assert_eq!(found[0].connected, Some(true));

        let quiet = tempfile::tempdir().unwrap();
        process(quiet.path(), 70, "input-leapc", &["input-leapc"], None);
        assert_eq!(scan_proc(quiet.path())[0].connected, None);
    }

    #[test]
    fn the_mode_turned_on_and_a_new_link_are_moments_to_look_and_nothing_else() {
        let none: &[String] = &[];
        let a = &["a".to_owned()][..];
        let ab = &["a".to_owned(), "b".to_owned()][..];
        assert_eq!(
            moment((false, none), (true, none)),
            Some("the multi-PC mode is on")
        );
        assert_eq!(moment((true, none), (true, a)), Some("a machine linked"));
        assert_eq!(moment((true, a), (true, ab)), Some("a machine linked"));
        // The same state again, a link lost, the mode turned off: nothing to look at.
        assert_eq!(moment((true, a), (true, a)), None);
        assert_eq!(moment((true, ab), (true, a)), None);
        assert_eq!(moment((true, a), (false, none)), None);
        assert_eq!(moment((false, none), (false, none)), None);
    }

    /// By hand, on a real machine: `cargo test -p lbm-agent --lib -- --ignored
    /// --nocapture what_runs_here`. Reads `/proc` only.
    #[test]
    #[ignore]
    fn what_runs_here() {
        for running in scan() {
            println!("{running:?}");
        }
        for warning in lbm_ipc::sharing::warnings(&scan()) {
            println!("{warning}");
        }
    }

    #[test]
    fn the_default_port_names_the_end() {
        assert_eq!(role_from_ports(24800, 40000), Some(Role::Server));
        assert_eq!(role_from_ports(40000, 24800), Some(Role::Client));
        assert_eq!(role_from_ports(40000, 443), None);
    }
}
