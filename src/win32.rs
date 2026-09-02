//! Minimal Win32 bindings used by Rearguard.
//!
//! The unsafe ABI surface stays in this module. The rest of the application
//! works with owned handles, Rust strings, and `std::io::Error` values.

use std::ffi::c_void;
use std::io;
use std::mem::size_of;
use std::ptr::{null, null_mut};

type Bool = i32;
type Dword = u32;
type Handle = *mut c_void;
type ScHandle = *mut c_void;

const FALSE: Bool = 0;
const MAX_PATH: usize = 260;

const ERROR_NO_MORE_FILES: i32 = 18;
const ERROR_SERVICE_DOES_NOT_EXIST: i32 = 1060;

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

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcessEntry32W {
    dw_size: Dword,
    cnt_usage: Dword,
    process_id: Dword,
    default_heap_id: usize,
    module_id: Dword,
    thread_count: Dword,
    parent_process_id: Dword,
    base_priority: i32,
    flags: Dword,
    executable_file: [u16; MAX_PATH],
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
struct ServiceStatus {
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
    fn CreateToolhelp32Snapshot(flags: Dword, process_id: Dword) -> Handle;
    fn Process32FirstW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
    fn Process32NextW(snapshot: Handle, entry: *mut ProcessEntry32W) -> Bool;
    fn OpenProcess(desired_access: Dword, inherit_handle: Bool, process_id: Dword) -> Handle;
    fn TerminateProcess(process: Handle, exit_code: u32) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenSCManagerW(
        machine_name: *const u16,
        database_name: *const u16,
        desired_access: Dword,
    ) -> ScHandle;
    fn OpenServiceW(
        service_manager: ScHandle,
        service_name: *const u16,
        desired_access: Dword,
    ) -> ScHandle;
    fn QueryServiceStatus(service: ScHandle, status: *mut ServiceStatus) -> Bool;
    fn ChangeServiceConfigW(
        service: ScHandle,
        service_type: Dword,
        start_type: Dword,
        error_control: Dword,
        binary_path_name: *const u16,
        load_order_group: *const u16,
        tag_id: *mut Dword,
        dependencies: *const u16,
        service_start_name: *const u16,
        password: *const u16,
        display_name: *const u16,
    ) -> Bool;
    fn ControlService(service: ScHandle, control: Dword, status: *mut ServiceStatus) -> Bool;
    fn CloseServiceHandle(service: ScHandle) -> Bool;
}

struct KernelHandle(Handle);

impl KernelHandle {
    fn from_snapshot(raw: Handle) -> io::Result<Self> {
        if raw as isize == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }

    fn from_nullable(raw: Handle) -> io::Result<Self> {
        if raw.is_null() {
            Err(io::Error::last_os_error())
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
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

struct OwnedServiceHandle(ScHandle);

impl OwnedServiceHandle {
    fn from_nullable(raw: ScHandle) -> io::Result<Self> {
        if raw.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(raw))
        }
    }

    fn raw(&self) -> ScHandle {
        self.0
    }
}

impl Drop for OwnedServiceHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseServiceHandle(self.0);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceState {
    Running,
    Stopped,
    Pending,
}

pub(crate) struct Service {
    handle: OwnedServiceHandle,
}

impl Service {
    pub(crate) fn open(service_name: &str) -> io::Result<Option<Self>> {
        let service_name = to_wide_null(service_name)?;
        let service_manager = OwnedServiceHandle::from_nullable(unsafe {
            OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT)
        })?;
        let raw_service = unsafe {
            OpenServiceW(
                service_manager.raw(),
                service_name.as_ptr(),
                SERVICE_CHANGE_CONFIG | SERVICE_QUERY_STATUS | SERVICE_STOP,
            )
        };

        if raw_service.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST) {
                Ok(None)
            } else {
                Err(error)
            }
        } else {
            Ok(Some(Self {
                handle: OwnedServiceHandle(raw_service),
            }))
        }
    }

    pub(crate) fn state(&self) -> io::Result<ServiceState> {
        let mut status = ServiceStatus::default();
        if unsafe { QueryServiceStatus(self.handle.raw(), &mut status) } == FALSE {
            return Err(io::Error::last_os_error());
        }

        Ok(match status.current_state {
            SERVICE_RUNNING => ServiceState::Running,
            SERVICE_STOPPED => ServiceState::Stopped,
            _ => ServiceState::Pending,
        })
    }

    pub(crate) fn disable(&self) -> io::Result<()> {
        let changed = unsafe {
            ChangeServiceConfigW(
                self.handle.raw(),
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

        bool_result(changed)
    }

    pub(crate) fn stop(&self) -> io::Result<()> {
        let mut status = ServiceStatus::default();
        bool_result(unsafe { ControlService(self.handle.raw(), SERVICE_CONTROL_STOP, &mut status) })
    }
}

pub(crate) fn terminate_processes_by_exact_name(targets: &[&str]) -> io::Result<()> {
    let snapshot =
        KernelHandle::from_snapshot(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })?;
    let mut entry = ProcessEntry32W::default();

    if unsafe { Process32FirstW(snapshot.raw(), &mut entry) } == FALSE {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES) {
            Ok(())
        } else {
            Err(error)
        };
    }

    let mut first_error = None;
    loop {
        if targets
            .iter()
            .any(|target| executable_name_matches(&entry.executable_file, target))
        {
            if let Err(error) = terminate_process(entry.process_id) {
                first_error.get_or_insert(error);
            }
        }

        if unsafe { Process32NextW(snapshot.raw(), &mut entry) } == FALSE {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(ERROR_NO_MORE_FILES) {
                return Err(error);
            }
            break;
        }
    }

    first_error.map_or(Ok(()), Err)
}

fn terminate_process(process_id: Dword) -> io::Result<()> {
    let process =
        KernelHandle::from_nullable(unsafe { OpenProcess(PROCESS_TERMINATE, FALSE, process_id) })?;
    bool_result(unsafe { TerminateProcess(process.raw(), 1) })
}

fn executable_name_matches(buffer: &[u16], target: &str) -> bool {
    let name_length = buffer
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(buffer.len());
    target
        .encode_utf16()
        .eq(buffer[..name_length].iter().copied())
}

fn to_wide_null(value: &str) -> io::Result<Vec<u16>> {
    if value.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Windows strings cannot contain an interior null",
        ));
    }

    Ok(value.encode_utf16().chain([0]).collect())
}

fn bool_result(result: Bool) -> io::Result<()> {
    if result == FALSE {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn compares_exact_utf16_executable_names() {
        let mut buffer = [0; MAX_PATH];
        let encoded: Vec<_> = "LeagueClient.exe".encode_utf16().collect();
        buffer[..encoded.len()].copy_from_slice(&encoded);

        assert!(executable_name_matches(&buffer, "LeagueClient.exe"));
        assert!(!executable_name_matches(&buffer, "leagueclient.exe"));
        assert!(!executable_name_matches(&buffer, "LeagueClient"));
    }

    #[test]
    fn enumerates_processes_without_terminating_anything() {
        terminate_processes_by_exact_name(&[]).unwrap();
    }

    #[test]
    fn recognizes_a_missing_service_without_changing_configuration() {
        let service = Service::open("__rearguard_test_service_does_not_exist__");
        assert!(matches!(service, Ok(None)));
    }

    #[test]
    fn creates_null_terminated_windows_strings() {
        assert_eq!(to_wide_null("vgc").unwrap(), vec![118, 103, 99, 0]);
        assert_eq!(to_wide_null("A💣").unwrap(), vec![65, 0xd83d, 0xdca3, 0]);
        assert!(to_wide_null("bad\0name").is_err());
    }

    #[test]
    fn win32_structures_match_the_x64_abi() {
        assert_eq!(size_of::<Handle>(), 8);
        assert_eq!(size_of::<ProcessEntry32W>(), 568);
        assert_eq!(offset_of!(ProcessEntry32W, default_heap_id), 16);
        assert_eq!(offset_of!(ProcessEntry32W, executable_file), 44);
        assert_eq!(size_of::<ServiceStatus>(), 28);
    }

    #[test]
    fn uses_documented_win32_access_and_state_constants() {
        assert_eq!(TH32CS_SNAPPROCESS, 0x0000_0002);
        assert_eq!(PROCESS_TERMINATE, 0x0001);
        assert_eq!(SC_MANAGER_CONNECT, 0x0001);
        assert_eq!(
            SERVICE_CHANGE_CONFIG | SERVICE_QUERY_STATUS | SERVICE_STOP,
            0x0026
        );
        assert_eq!(SERVICE_DISABLED, 0x0000_0004);
    }
}
