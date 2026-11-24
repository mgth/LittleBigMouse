//! Launching `lbm-inject` into a session with the service's SYSTEM token.
//!
//! The token is this process's own, duplicated and moved into the target session
//! (`TokenSessionId`, which needs SeTcbPrivilege: LocalSystem has it). The injector is
//! started on `winsta0\default` and moves itself to the input desktop when it needs
//! to (`lbm_hook`'s `input_desktop`).
//!
//! Its only input is an anonymous pipe: the read end is its standard input, the only
//! handle it inherits (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`), and the write end stays
//! here. No named object exists that another process could open or create first.

use std::ffi::c_void;
use std::fs::File;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::Path;
use std::sync::mpsc::Sender;

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, SetHandleInformation, BOOLEAN, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT,
};
use windows::Win32::Security::{
    DuplicateTokenEx, SecurityImpersonation, SetTokenInformation, TokenPrimary, TokenSessionId,
    TOKEN_ADJUST_DEFAULT, TOKEN_ADJUST_SESSIONID, TOKEN_ALL_ACCESS, TOKEN_ASSIGN_PRIMARY,
    TOKEN_DUPLICATE, TOKEN_QUERY,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::RemoteDesktop::WTSGetActiveConsoleSessionId;
use windows::Win32::System::Threading::{
    CreateProcessAsUserW, DeleteProcThreadAttributeList, GetCurrentProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, RegisterWaitForSingleObject,
    TerminateProcess, UnregisterWaitEx, UpdateProcThreadAttribute, CREATE_NO_WINDOW,
    EXTENDED_STARTUPINFO_PRESENT, INFINITE, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW, WT_EXECUTEONLYONCE,
};

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
    /// Asks the system to send `session` on `exits` when the process ends: no thread
    /// waits for it, and nothing polls.
    pub fn notify_exit(&mut self, exits: Sender<u32>) -> Result<(), String> {
        let context = Box::into_raw(Box::new((self.session, exits)));
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
        use windows::Win32::Foundation::WAIT_OBJECT_0;
        use windows::Win32::System::Threading::WaitForSingleObject;
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
    let context = unsafe { Box::from_raw(context as *mut (u32, Sender<u32>)) };
    let _ = context.1.send(context.0);
}

/// Launches `injector` into `session` with this process's token moved there.
/// `log` becomes its standard error.
pub fn launch(injector: &Path, session: u32, log: Option<&File>) -> Result<Injector, String> {
    let token = system_token_in(session)?;

    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    // SAFETY: out pointers to locals; neither end is inheritable yet.
    unsafe { CreatePipe(&mut read, &mut write, None, 0) }
        .map_err(|e| format!("CreatePipe: {e}"))?;
    let read = Owned(read);
    // SAFETY: the handle was just made; from here the File owns it.
    let input = unsafe { File::from_raw_handle(write.0 as RawHandle) };
    // SAFETY: a handle this function owns.
    unsafe { SetHandleInformation(read.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
        .map_err(|e| format!("SetHandleInformation: {e}"))?;

    let error = match log {
        Some(log) => {
            use std::os::windows::io::AsRawHandle;
            let handle = HANDLE(log.as_raw_handle());
            // The log is inheritable for the launch only: the inheritance is reset
            // at the end of this function, whatever happens.
            // SAFETY: a handle owned by the caller's File, alive for this call.
            unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAG_INHERIT) }
                .map_err(|e| format!("SetHandleInformation(log): {e}"))?;
            Some(handle)
        }
        None => None,
    };
    let result = create(injector, &token, &read, error);
    if let Some(handle) = error {
        // SAFETY: as above.
        let _ = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0)) };
    }
    let (process, pid) = result?;
    Ok(Injector {
        session,
        pid,
        input,
        process,
        wait: None,
    })
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

fn create(
    injector: &Path,
    token: &Owned,
    input: &Owned,
    error: Option<HANDLE>,
) -> Result<(Owned, u32), String> {
    let mut inherited = vec![input.0];
    inherited.extend(error);

    let mut size = 0usize;
    // SAFETY: the documented size query; it fails with the size filled in.
    let _ = unsafe {
        InitializeProcThreadAttributeList(LPPROC_THREAD_ATTRIBUTE_LIST::default(), 1, 0, &mut size)
    };
    let mut list_storage = vec![0u8; size];
    let list = LPPROC_THREAD_ATTRIBUTE_LIST(list_storage.as_mut_ptr().cast());
    // SAFETY: storage of the size asked for.
    unsafe { InitializeProcThreadAttributeList(list, 1, 0, &mut size) }
        .map_err(|e| format!("InitializeProcThreadAttributeList: {e}"))?;
    // SAFETY: `inherited` outlives the list, which is deleted below.
    let updated = unsafe {
        UpdateProcThreadAttribute(
            list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(inherited.as_ptr() as *const c_void),
            inherited.len() * size_of::<HANDLE>(),
            None,
            None,
        )
    };
    let result = updated
        .map_err(|e| format!("UpdateProcThreadAttribute: {e}"))
        .and_then(|()| spawn(injector, token, input, error, list));
    // SAFETY: initialised above.
    unsafe { DeleteProcThreadAttributeList(list) };
    result
}

fn spawn(
    injector: &Path,
    token: &Owned,
    input: &Owned,
    error: Option<HANDLE>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
) -> Result<(Owned, u32), String> {
    let application: Vec<u16> = injector.as_os_str().encode_wide().chain([0]).collect();
    let mut command_line: Vec<u16> = format!("\"{}\"", injector.display())
        .encode_utf16()
        .chain([0])
        .collect();
    let mut desktop: Vec<u16> = "winsta0\\default".encode_utf16().chain([0]).collect();

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.lpDesktop = PWSTR(desktop.as_mut_ptr());
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = input.0;
    startup.StartupInfo.hStdOutput = HANDLE::default();
    startup.StartupInfo.hStdError = error.unwrap_or_default();
    startup.lpAttributeList = list;

    let mut information = PROCESS_INFORMATION::default();
    // SAFETY: every buffer outlives the call; the application path is absolute, so
    // no search path is consulted.
    unsafe {
        CreateProcessAsUserW(
            token.0,
            PCWSTR(application.as_ptr()),
            PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW,
            None,
            PCWSTR::null(),
            &startup.StartupInfo,
            &mut information,
        )
    }
    .map_err(|e| format!("CreateProcessAsUserW: {e}"))?;
    drop(Owned(information.hThread));
    Ok((Owned(information.hProcess), information.dwProcessId))
}
