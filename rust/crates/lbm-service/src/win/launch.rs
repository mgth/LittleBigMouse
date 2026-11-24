//! Launching `lbm-inject` into a session with the service's SYSTEM token.
//!
//! The token is this process's own, duplicated and moved into the target session
//! (`TokenSessionId`, which needs SeTcbPrivilege: LocalSystem has it). The injector is
//! started on `winsta0\default` and moves itself to the input desktop when it needs
//! to (`lbm_hook`'s `input_desktop`).
//!
//! Handles are not inherited across sessions, and the service is in session 0, so the
//! input cannot be an inherited pipe. It is a named pipe made for this launch: a name
//! nobody can guess (`lbm-inject-` and 128 random bits), a DACL that grants SYSTEM
//! alone, a first and only instance, no remote client. The injector checks that the
//! pipe's server is the process that launched it (`--server`), and this side that its
//! client is the process it launched. A second pipe, `<name>-log`, carries the
//! injector's diagnostics back; nothing read from it is acted on.
//!
//! Nothing is inherited: `bInheritHandles` is false.

use std::ffi::c_void;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, BOOLEAN, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL, WAIT_OBJECT_0,
};
use windows::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
use windows::Win32::Security::{
    DuplicateTokenEx, SecurityImpersonation, SetTokenInformation, TokenPrimary, TokenSessionId,
    PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_SESSIONID,
    TOKEN_ALL_ACCESS, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND, PIPE_ACCESS_OUTBOUND,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, GetCurrentProcess, GetCurrentProcessId, OpenProcessToken,
    RegisterWaitForSingleObject, TerminateProcess, UnregisterWaitEx, WaitForSingleObject,
    CREATE_NO_WINDOW, INFINITE, PROCESS_INFORMATION, STARTUPINFOW, WT_EXECUTEONLYONCE,
};

/// How long a launched injector has to open its input.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// SYSTEM alone, nothing inherited.
const PIPE_SDDL: PCWSTR = w!("D:P(A;;GA;;;SY)");

/// A handle closed on drop.
struct Owned(HANDLE);

impl Drop for Owned {
    fn drop(&mut self) {
        if !self.0.is_invalid() && !self.0 .0.is_null() {
            // SAFETY: a handle this value owns.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

// SAFETY: kernel handles are usable from any thread.
unsafe impl Send for Owned {}

/// The session at the console, `None` when the console is attached to none.
pub fn console_session() -> Option<u32> {
    // SAFETY: no arguments.
    let id = unsafe { WTSGetActiveConsoleSessionId() };
    (id != u32::MAX).then_some(id)
}

/// A running injector.
pub struct Injector {
    pub session: u32,
    pub pid: u32,
    /// Its input. Dropping it ends the stream: the injector releases what is pressed
    /// and leaves.
    pub input: File,
    process: Owned,
    wait: Option<HANDLE>,
}

// SAFETY: the wait handle is only used to unregister.
unsafe impl Send for Injector {}

impl Injector {
    /// Asks the system to send `launch` on `exits` when the process ends: no thread
    /// waits for it, and nothing polls. The launch number, not the session, tells this
    /// injector's exit from that of an older one of the same session.
    pub fn notify_exit(&mut self, launch: u64, exits: Sender<u64>) -> Result<(), String> {
        let context = Box::into_raw(Box::new((launch, exits)));
        let mut wait = HANDLE::default();
        // SAFETY: the context outlives the wait: it is freed by the callback, which
        // runs once (WT_EXECUTEONLYONCE), or below when the registration fails.
        let registered = unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                self.process.0,
                Some(exited),
                Some(context as *const c_void),
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };
        if let Err(error) = registered {
            // SAFETY: never handed to a callback.
            drop(unsafe { Box::from_raw(context) });
            return Err(format!("RegisterWaitForSingleObject: {error}"));
        }
        self.wait = Some(wait);
        Ok(())
    }

    /// Ends the stream and, if the injector has not left after `grace`, ends the
    /// process. Its releases were in the stream it read; a process killed here had
    /// stopped reading.
    pub fn stop(self, grace: std::time::Duration) {
        let Injector {
            input,
            process,
            wait,
            ..
        } = self;
        drop(input);
        // SAFETY: a process handle this value owns.
        let ended = unsafe { WaitForSingleObject(process.0, grace.as_millis() as u32) };
        if ended != WAIT_OBJECT_0 {
            // SAFETY: as above.
            let _ = unsafe { TerminateProcess(process.0, 1) };
        }
        if let Some(wait) = wait {
            // Waits for a callback in flight: the process handle is closed after.
            // SAFETY: a wait this value registered.
            let _ = unsafe { UnregisterWaitEx(wait, HANDLE(-1isize as *mut c_void)) };
        }
    }
}

unsafe extern "system" fn exited(context: *mut c_void, _timed_out: BOOLEAN) {
    // SAFETY: the box handed over by `notify_exit`, taken back once.
    let context = unsafe { Box::from_raw(context as *mut (u64, Sender<u64>)) };
    let _ = context.1.send(context.0);
}

/// Launches `injector` into `session` with this process's token moved there, and
/// waits until it has opened its input. Each line of its diagnostics goes to `log`.
pub fn launch(
    injector: &Path,
    session: u32,
    log: impl Fn(&str) + Send + 'static,
) -> Result<Injector, String> {
    let token = system_token_in(session)?;
    let name = random_name()?;
    let input = create_pipe(&name, PIPE_ACCESS_OUTBOUND.0)?;
    let diagnostics = create_pipe(&format!("{name}-log"), PIPE_ACCESS_INBOUND.0)?;
    let (process, pid) = spawn(injector, &token, &name)?;
    let process = Owned(process);

    let started = (|| {
        connect(&input, pid, &name)?;
        Ok::<_, String>(())
    })();
    if let Err(error) = started {
        // SAFETY: a process this function started.
        let _ = unsafe { TerminateProcess(process.0, 1) };
        return Err(error);
    }
    // The log is a convenience: an injector that did not open it still injects.
    let log_name = format!("{name}-log");
    std::thread::spawn(move || {
        if connect(&diagnostics, pid, &log_name).is_ok() {
            for line in BufReader::new(diagnostics).lines() {
                match line {
                    Ok(line) => log(&line),
                    Err(_) => break,
                }
            }
        }
    });
    Ok(Injector {
        session,
        pid,
        input,
        process,
        wait: None,
    })
}

/// `lbm-inject-` and 32 hex digits from the system's random generator.
fn random_name() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    // SAFETY: a buffer of its own length.
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }
        .ok()
        .map_err(|e| format!("BCryptGenRandom: {e}"))?;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!("lbm-inject-{hex}"))
}

/// The first and only instance of `\\.\pipe\<name>`, open to SYSTEM alone.
fn create_pipe(name: &str, direction: u32) -> Result<File, String> {
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: a constant SDDL string; the descriptor is freed below.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PIPE_SDDL,
            SDDL_REVISION_1,
            &mut descriptor,
            None,
        )
    }
    .map_err(|e| format!("pipe security: {e}"))?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: false.into(),
    };
    let path: Vec<u16> = format!(r"\\.\pipe\{name}")
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: a NUL-terminated name and attributes alive for the call.
    let pipe = unsafe {
        CreateNamedPipeW(
            PCWSTR(path.as_ptr()),
            windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(
                direction | FILE_FLAG_FIRST_PIPE_INSTANCE.0,
            ),
            PIPE_TYPE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            64 * 1024,
            64 * 1024,
            0,
            Some(&attributes),
        )
    };
    // SAFETY: allocated by the conversion above.
    unsafe {
        let _ = LocalFree(HLOCAL(descriptor.0));
    }
    if pipe.is_invalid() {
        return Err(format!(
            "pipe {name}: {}",
            windows::core::Error::from_win32()
        ));
    }
    // SAFETY: a handle just made, owned by the File from here.
    Ok(unsafe { File::from_raw_handle(pipe.0 as RawHandle) })
}

/// Waits for the injector to open `pipe`, at most [`CONNECT_TIMEOUT`], and checks that
/// the process that did is the one launched. The wait is a blocking connect on a
/// thread of its own; on time out, this side connects to its own pipe to end it.
fn connect(pipe: &File, pid: u32, name: &str) -> Result<(), String> {
    let handle = pipe.as_raw_handle() as usize;
    let (done, connected) = mpsc::channel();
    let waiter = std::thread::spawn(move || {
        // SAFETY: the pipe outlives this thread, which is joined below.
        let result = unsafe { ConnectNamedPipe(HANDLE(handle as *mut c_void), None) };
        let _ = done.send(match result {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERROR_PIPE_CONNECTED.to_hresult() => Ok(()),
            Err(error) => Err(error.to_string()),
        });
    });
    let outcome = match connected.recv_timeout(CONNECT_TIMEOUT) {
        Ok(result) => result,
        Err(_) => {
            // Ends the blocked connect; whatever it connected is closed at once.
            // Read for an outbound pipe, write for an inbound one: the side that
            // matches is the one that opens.
            let path = format!(r"\\.\pipe\{name}");
            let _ = std::fs::OpenOptions::new()
                .read(true)
                .open(&path)
                .or_else(|_| std::fs::OpenOptions::new().write(true).open(&path));
            Err(format!("the injector did not open {name} in time"))
        }
    };
    let _ = waiter.join();
    outcome?;
    let mut client = 0u32;
    // SAFETY: a pipe handle owned by `pipe`, an out pointer to a local.
    unsafe { GetNamedPipeClientProcessId(HANDLE(pipe.as_raw_handle()), &mut client) }
        .map_err(|e| format!("{name}: no client process id: {e}"))?;
    if client != pid {
        return Err(format!("{name} was opened by process {client}, not {pid}"));
    }
    Ok(())
}

fn system_token_in(session: u32) -> Result<Owned, String> {
    let mut own = HANDLE::default();
    // SAFETY: an out pointer to a local.
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE
                | TOKEN_QUERY
                | TOKEN_ASSIGN_PRIMARY
                | TOKEN_ADJUST_DEFAULT
                | TOKEN_ADJUST_SESSIONID,
            &mut own,
        )
    }
    .map_err(|e| format!("OpenProcessToken: {e}"))?;
    let own = Owned(own);
    let mut primary = HANDLE::default();
    // SAFETY: a token this function owns, an out pointer to a local.
    unsafe {
        DuplicateTokenEx(
            own.0,
            TOKEN_ALL_ACCESS,
            None,
            SecurityImpersonation,
            TokenPrimary,
            &mut primary,
        )
    }
    .map_err(|e| format!("DuplicateTokenEx: {e}"))?;
    let primary = Owned(primary);
    // SAFETY: a u32 and its size.
    unsafe {
        SetTokenInformation(
            primary.0,
            TokenSessionId,
            &session as *const u32 as *const c_void,
            size_of::<u32>() as u32,
        )
    }
    .map_err(|e| format!("SetTokenInformation(TokenSessionId), SYSTEM only: {e}"))?;
    Ok(primary)
}

fn spawn(injector: &Path, token: &Owned, name: &str) -> Result<(HANDLE, u32), String> {
    let application: Vec<u16> = injector.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: no arguments.
    let server = unsafe { GetCurrentProcessId() };
    let mut command_line: Vec<u16> =
        format!("\"{}\" --pipe {name} --server {server}", injector.display())
            .encode_utf16()
            .chain([0])
            .collect();
    let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain([0]).collect();
    let startup = STARTUPINFOW {
        cb: size_of::<STARTUPINFOW>() as u32,
        lpDesktop: PWSTR(desktop.as_mut_ptr()),
        ..Default::default()
    };
    let mut information = PROCESS_INFORMATION::default();
    // SAFETY: every buffer outlives the call; the application path is absolute, so
    // no search path is consulted; nothing is inherited.
    unsafe {
        CreateProcessAsUserW(
            token.0,
            PCWSTR(application.as_ptr()),
            PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            false,
            CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup,
            &mut information,
        )
    }
    .map_err(|e| format!("CreateProcessAsUserW: {e}"))?;
    drop(Owned(information.hThread));
    Ok((information.hProcess, information.dwProcessId))
}
