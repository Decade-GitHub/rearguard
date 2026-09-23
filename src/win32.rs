//! Handwritten Win32 ABI surface used by the production executable.

use core::ffi::c_void;
use core::mem::size_of;
use core::ptr::{null, null_mut};

use crate::logic::{AlertKind, ServiceState, alert_kind_for_process, select_alert};

pub(crate) type Handle = *mut c_void;
type Bool = i32;
type Dword = u32;
type ScHandle = Handle;

const FALSE: Bool = 0;
pub(crate) const MAX_PATH: usize = 260;
const ERROR_NO_MORE_FILES: Dword = 18;
const ERROR_SERVICE_DOES_NOT_EXIST: Dword = 1060;
const ERROR_INSUFFICIENT_BUFFER: Dword = 122;
const ERROR_INVALID_PARAMETER: Dword = 87;
const TH32CS_SNAPPROCESS: Dword = 0x0000_0002;
const PROCESS_TERMINATE: Dword = 0x0001;
const SC_MANAGER_CONNECT: Dword = 0x0001;
const SERVICE_CHANGE_CONFIG: Dword = 0x0002;
const SERVICE_QUERY_STATUS: Dword = 0x0004;
const SERVICE_STOP: Dword = 0x0020;
const SERVICE_CONTROL_STOP: Dword = 0x0000_0001;
const SERVICE_STOPPED: Dword = 0x0000_0001;
const SERVICE_RUNNING: Dword = 0x0000_0004;
const SERVICE_DISABLED: Dword = 0x0000_0004;
const SERVICE_NO_CHANGE: Dword = 0xffff_ffff;
const MB_ICONERROR: Dword = 0x0000_0010;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Error {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    pub(crate) operation: &'static str,
    pub(crate) code: Dword,
}

impl Error {
    pub(crate) const fn new(operation: &'static str, code: Dword) -> Self {
        Self { operation, code }
    }

    fn last(operation: &'static str) -> Self {
        Self::new(operation, unsafe { GetLastError() })
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct ProcessEntry32W {
    dw_size: Dword,
    cnt_usage: Dword,
    process_id: Dword,
    pub(crate) default_heap_id: usize,
    module_id: Dword,
    thread_count: Dword,
    parent_process_id: Dword,
    base_priority: i32,
    flags: Dword,
    pub(crate) executable_file: [u16; MAX_PATH],
}

impl Default for ProcessEntry32W {
    fn default() -> Self {
        Self {
            dw_size: size_of::<Self>() as Dword,
            cnt_usage: 0,
            process_id: 0,
            default_heap_id: 0,
            module_id: 0,
            thread_count: 0,
            parent_process_id: 0,
            base_priority: 0,
            flags: 0,
            executable_file: [0; MAX_PATH],
        }
    }
}

#[repr(C)]
#[derive(Default)]
pub(crate) struct ServiceStatus {
    service_type: Dword,
    current_state: Dword,
    controls_accepted: Dword,
    win32_exit_code: Dword,
    service_specific_exit_code: Dword,
    check_point: Dword,
    wait_hint: Dword,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCommandLineW() -> *const u16;
    fn GetLastError() -> Dword;
    fn Sleep(milliseconds: Dword);
    fn ExitProcess(exit_code: Dword);
    fn CreateToolhelp32Snapshot(flags: Dword, process_id: Dword) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
    fn OpenProcess(desired_access: Dword, inherit_handle: Bool, process_id: Dword) -> Handle;
    fn TerminateProcess(process: Handle, exit_code: Dword) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
    #[cfg(debug_assertions)]
    fn GetStdHandle(which: Dword) -> Handle;
    #[cfg(debug_assertions)]
    fn WriteFile(
        handle: Handle,
        buffer: *const u8,
        bytes: Dword,
        written: *mut Dword,
        overlapped: Handle,
    ) -> Bool;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenSCManagerW(machine: *const u16, database: *const u16, access: Dword) -> ScHandle;
    fn OpenServiceW(manager: ScHandle, name: *const u16, access: Dword) -> ScHandle;
    fn QueryServiceStatus(service: ScHandle, status: *mut ServiceStatus) -> Bool;
    fn ChangeServiceConfigW(
        service: ScHandle,
        service_type: Dword,
        start_type: Dword,
        error_control: Dword,
        binary_path: *const u16,
        load_group: *const u16,
        tag: *mut Dword,
        dependencies: *const u16,
        account: *const u16,
        password: *const u16,
        display_name: *const u16,
    ) -> Bool;
    fn ControlService(service: ScHandle, control: Dword, status: *mut ServiceStatus) -> Bool;
    fn CloseServiceHandle(service: ScHandle) -> Bool;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(window: Handle, text: *const u16, title: *const u16, flags: Dword) -> i32;
}

pub(crate) fn exit(code: Dword) -> ! {
    unsafe { ExitProcess(code) };
    loop {
        core::hint::spin_loop();
    }
}

pub(crate) fn sleep(milliseconds: Dword) {
    unsafe { Sleep(milliseconds) };
}

pub(crate) fn command_line() -> Result<&'static [u16], Error> {
    let pointer = unsafe { GetCommandLineW() };
    if pointer.is_null() {
        return Err(Error::last("GetCommandLineW"));
    }
    // Windows command lines are limited to 32,767 UTF-16 code units.
    for length in 0..32_768 {
        if unsafe { *pointer.add(length) } == 0 {
            return Ok(unsafe { core::slice::from_raw_parts(pointer, length) });
        }
    }
    Err(Error::new("GetCommandLineW", ERROR_INSUFFICIENT_BUFFER))
}

pub(crate) struct KernelHandle(Handle);

impl KernelHandle {
    fn from_snapshot(raw: Handle) -> Result<Self, Error> {
        if raw as isize == -1 {
            Err(Error::last("CreateToolhelp32Snapshot"))
        } else {
            Ok(Self(raw))
        }
    }

    fn from_nullable(raw: Handle, operation: &'static str) -> Result<Self, Error> {
        if raw.is_null() {
            Err(Error::last(operation))
        } else {
            Ok(Self(raw))
        }
    }

    fn raw(&self) -> Handle {
        self.0
    }
}

impl Drop for KernelHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub(crate) struct ServiceHandle(ScHandle);

impl ServiceHandle {
    fn from_nullable(raw: ScHandle, operation: &'static str) -> Result<Self, Error> {
        if raw.is_null() {
            Err(Error::last(operation))
        } else {
            Ok(Self(raw))
        }
    }
}

impl Drop for ServiceHandle {
    fn drop(&mut self) {
        unsafe { CloseServiceHandle(self.0) };
    }
}

pub(crate) struct Service {
    handle: ServiceHandle,
}

impl Service {
    pub(crate) fn open(name: &str) -> Result<Option<Self>, Error> {
        let mut wide = [0u16; 257];
        encode_ascii_nul(name, &mut wide)?;
        let manager = ServiceHandle::from_nullable(
            unsafe { OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT) },
            "OpenSCManagerW",
        )?;
        let service = unsafe {
            OpenServiceW(
                manager.0,
                wide.as_ptr(),
                SERVICE_CHANGE_CONFIG | SERVICE_QUERY_STATUS | SERVICE_STOP,
            )
        };
        if service.is_null() {
            let error = Error::last("OpenServiceW");
            if error.code == ERROR_SERVICE_DOES_NOT_EXIST {
                Ok(None)
            } else {
                Err(error)
            }
        } else {
            Ok(Some(Self {
                handle: ServiceHandle(service),
            }))
        }
    }

    pub(crate) fn state(&self) -> Result<ServiceState, Error> {
        let mut status = ServiceStatus::default();
        if unsafe { QueryServiceStatus(self.handle.0, &mut status) } == FALSE {
            return Err(Error::last("QueryServiceStatus"));
        }
        Ok(match status.current_state {
            SERVICE_RUNNING => ServiceState::Running,
            SERVICE_STOPPED => ServiceState::Stopped,
            _ => ServiceState::Pending,
        })
    }

    pub(crate) fn disable(&self) -> Result<(), Error> {
        let result = unsafe {
            ChangeServiceConfigW(
                self.handle.0,
                SERVICE_NO_CHANGE,
                SERVICE_DISABLED,
                SERVICE_NO_CHANGE,
                null(),
                null(),
                null_mut(),
                null(),
                null(),
                null(),
                null(),
            )
        };
        bool_result(result, "ChangeServiceConfigW")
    }

    pub(crate) fn stop(&self) -> Result<(), Error> {
        let mut status = ServiceStatus::default();
        bool_result(
            unsafe { ControlService(self.handle.0, SERVICE_CONTROL_STOP, &mut status) },
            "ControlService",
        )
    }
}

#[derive(Default)]
pub(crate) struct ProcessEnforcementResult {
    pub(crate) alert: Option<AlertKind>,
    pub(crate) first_error: Option<Error>,
}

impl ProcessEnforcementResult {
    fn record_error(&mut self, error: Error) {
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }
}

pub(crate) fn terminate_processes_by_exact_name(targets: &[&str]) -> ProcessEnforcementResult {
    let mut result = ProcessEnforcementResult::default();
    let snapshot = match KernelHandle::from_snapshot(unsafe {
        CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    }) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            result.record_error(error);
            return result;
        }
    };
    let mut entry = ProcessEntry32W::default();
    if unsafe { Process32FirstW(snapshot.raw(), &mut entry) } == FALSE {
        let error = Error::last("Process32FirstW");
        if error.code != ERROR_NO_MORE_FILES {
            result.record_error(error);
        }
        return result;
    }

    loop {
        if let Some(&target) = targets
            .iter()
            .find(|target| executable_name_matches(&entry.executable_file, target))
        {
            match terminate_process(entry.process_id) {
                Ok(()) => {
                    result.alert = select_alert(result.alert, alert_kind_for_process(target));
                }
                Err(error) => result.record_error(error),
            }
        }
        if unsafe { Process32NextW(snapshot.raw(), &mut entry) } == FALSE {
            let error = Error::last("Process32NextW");
            if error.code != ERROR_NO_MORE_FILES {
                result.record_error(error);
            }
            break;
        }
    }
    result
}

fn terminate_process(process_id: Dword) -> Result<(), Error> {
    let process = KernelHandle::from_nullable(
        unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, process_id) },
        "OpenProcess",
    )?;
    bool_result(unsafe { TerminateProcess(process.raw(), 1) }, "TerminateProcess")
}

pub(crate) fn executable_name_matches(buffer: &[u16], target: &str) -> bool {
    let length = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    let bytes = target.as_bytes();
    length == bytes.len()
        && buffer[..length]
            .iter()
            .zip(bytes)
            .all(|(&wide, &byte)| wide == byte as u16)
}

pub(crate) fn encode_ascii_nul(value: &str, output: &mut [u16]) -> Result<usize, Error> {
    let bytes = value.as_bytes();
    if bytes.len() >= output.len() {
        return Err(Error::new("UTF-16 buffer", ERROR_INSUFFICIENT_BUFFER));
    }
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == 0 || !byte.is_ascii() {
            return Err(Error::new("UTF-16 string", ERROR_INVALID_PARAMETER));
        }
        output[index] = byte as u16;
    }
    output[bytes.len()] = 0;
    Ok(bytes.len() + 1)
}

pub(crate) fn show_error_message_box(message: &str, title: &str) -> Result<(), Error> {
    let mut wide_message = [0u16; 64];
    let mut wide_title = [0u16; 16];
    encode_ascii_nul(message, &mut wide_message)?;
    encode_ascii_nul(title, &mut wide_title)?;
    if unsafe { MessageBoxW(null_mut(), wide_message.as_ptr(), wide_title.as_ptr(), MB_ICONERROR) }
        == 0
    {
        Err(Error::last("MessageBoxW"))
    } else {
        Ok(())
    }
}

fn bool_result(result: Bool, operation: &'static str) -> Result<(), Error> {
    if result == FALSE {
        Err(Error::last(operation))
    } else {
        Ok(())
    }
}

#[cfg(debug_assertions)]
pub(crate) fn report_error(error: Error) {
    let mut buffer = [0u8; 160];
    let mut length = copy_bytes(&mut buffer, b"rearguard: ");
    length += copy_bytes(&mut buffer[length..], error.operation.as_bytes());
    length += copy_bytes(&mut buffer[length..], b": 0x");
    for shift in (0..8).rev() {
        let digit = ((error.code >> (shift * 4)) & 15) as u8;
        buffer[length] = if digit < 10 { b'0' + digit } else { b'A' + digit - 10 };
        length += 1;
    }
    length += copy_bytes(&mut buffer[length..], b"\r\n");
    write_stderr(&buffer[..length]);
}

#[cfg(not(debug_assertions))]
pub(crate) fn report_error(_: Error) {}

#[cfg(debug_assertions)]
pub(crate) fn report_message(message: &str) {
    write_stderr(b"rearguard: ");
    write_stderr(message.as_bytes());
    write_stderr(b"\r\n");
}

#[cfg(not(debug_assertions))]
pub(crate) fn report_message(_: &str) {}

#[cfg(debug_assertions)]
fn copy_bytes(destination: &mut [u8], source: &[u8]) -> usize {
    let length = core::cmp::min(destination.len(), source.len());
    destination[..length].copy_from_slice(&source[..length]);
    length
}

#[cfg(debug_assertions)]
fn write_stderr(bytes: &[u8]) {
    let stderr = unsafe { GetStdHandle(-12i32 as u32) };
    if stderr.is_null() || stderr as isize == -1 {
        return;
    }
    let mut written = 0;
    unsafe {
        WriteFile(
            stderr,
            bytes.as_ptr(),
            bytes.len() as u32,
            &mut written,
            null_mut(),
        )
    };
}
