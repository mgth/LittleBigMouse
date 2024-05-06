use std::mem;
use std::ptr;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use winapi::um::tlhelp32::{CreateToolhelp32Snapshot, Process32First, Process32Next, PROCESSENTRY32, TH32CS_SNAPPROCESS};
use winapi::um::handleapi::CloseHandle;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::psapi::GetProcessImageFileNameW;
use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
use winapi::shared::minwindef::{DWORD, MAX_PATH};
use winapi::shared::ntdef::NULL;

fn get_parent_pid(pid: DWORD) -> DWORD {
    let mut h: HANDLE = ptr::null_mut();
    let mut pe: PROCESSENTRY32 = unsafe { mem::zeroed() };
    let mut ppid: DWORD = 0;
    pe.dwSize = mem::size_of::<PROCESSENTRY32>() as DWORD;
    h = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if unsafe { Process32First(h, &mut pe) } != 0 {
        loop {
            if pe.th32ProcessID == pid {
                ppid = pe.th32ParentProcessID;
                break;
            }
            if unsafe { Process32Next(h, &mut pe) } == 0 {
                break;
            }
        }
    }
    unsafe { CloseHandle(h) };
    ppid
}


fn main() {
    println!("Hello, world!");
}
