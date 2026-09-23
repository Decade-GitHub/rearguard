use core::cell::UnsafeCell;
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};
use core::ptr::{null, null_mut};

use crate::logic::TASK_NAME;
use crate::win32::{Error, Handle};

type Bool = i32;
type Dword = u32;
const ERROR_INSUFFICIENT_BUFFER: Dword = 122;
const ERROR_INVALID_PARAMETER: Dword = 87;
const WAIT_OBJECT_0: Dword = 0;
const INFINITE: Dword = 0xffff_ffff;
const ERROR_BUSY: Dword = 170;

struct Buffers {
    system_path: [u16; 512],
    command_line: [u16; 32_768],
    executable: [u16; 32_768],
}

struct SharedBuffers(UnsafeCell<Buffers>);

// Access is serialized by BUFFER_IN_USE. Rearguard's entry thread is normally
// the sole caller, but the guard also makes this safe if that changes.
unsafe impl Sync for SharedBuffers {}

static BUFFERS: SharedBuffers = SharedBuffers(UnsafeCell::new(Buffers {
    system_path: [0; 512],
    command_line: [0; 32_768],
    executable: [0; 32_768],
}));
static BUFFER_IN_USE: AtomicBool = AtomicBool::new(false);

struct BufferGuard;

impl BufferGuard {
    fn acquire() -> Result<Self, Error> {
        if BUFFER_IN_USE
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            Err(Error::new("scheduled task buffers", ERROR_BUSY))
        } else {
            Ok(Self)
        }
    }
}

impl Drop for BufferGuard {
    fn drop(&mut self) {
        BUFFER_IN_USE.store(false, Ordering::Release);
    }
}

#[repr(C)]
pub(crate) struct StartupInfoW {
    cb: Dword,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: Dword,
    y: Dword,
    x_size: Dword,
    y_size: Dword,
    x_count_chars: Dword,
    y_count_chars: Dword,
    fill_attribute: Dword,
    flags: Dword,
    show_window: u16,
    reserved2_size: u16,
    reserved2: *mut u8,
    std_input: Handle,
    std_output: Handle,
    std_error: Handle,
}

#[repr(C)]
pub(crate) struct ProcessInformation {
    process: Handle,
    thread: Handle,
    process_id: Dword,
    thread_id: Dword,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLastError() -> Dword;
    fn GetModuleFileNameW(module: Handle, filename: *mut u16, capacity: Dword) -> Dword;
    fn GetSystemDirectoryW(buffer: *mut u16, capacity: Dword) -> Dword;
    fn CreateProcessW(
        application: *const u16,
        command_line: *mut u16,
        process_attributes: Handle,
        thread_attributes: Handle,
        inherit_handles: Bool,
        flags: Dword,
        environment: Handle,
        current_directory: *const u16,
        startup: *mut StartupInfoW,
        information: *mut ProcessInformation,
    ) -> Bool;
    fn WaitForSingleObject(handle: Handle, milliseconds: Dword) -> Dword;
    fn GetExitCodeProcess(handle: Handle, exit_code: *mut Dword) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
}

fn last_error(operation: &'static str) -> Error {
    Error::new(operation, unsafe { GetLastError() })
}

struct ProcessHandles {
    process: Handle,
    thread: Handle,
}

impl Drop for ProcessHandles {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.thread);
            CloseHandle(self.process);
        }
    }
}

struct WideBuilder<'a> {
    buffer: &'a mut [u16],
    length: usize,
}

impl<'a> WideBuilder<'a> {
    fn new(buffer: &'a mut [u16]) -> Self {
        Self { buffer, length: 0 }
    }

    fn with_length(buffer: &'a mut [u16], length: usize) -> Self {
        Self { buffer, length }
    }

    fn character(&mut self, character: u16) -> Result<(), Error> {
        if self.length == self.buffer.len() {
            return Err(Error::new("command line buffer", ERROR_INSUFFICIENT_BUFFER));
        }
        self.buffer[self.length] = character;
        self.length += 1;
        Ok(())
    }

    fn ascii(&mut self, text: &str) -> Result<(), Error> {
        for &byte in text.as_bytes() {
            if byte == 0 || !byte.is_ascii() {
                return Err(Error::new("command line text", ERROR_INVALID_PARAMETER));
            }
            self.character(byte as u16)?;
        }
        Ok(())
    }

    fn wide(&mut self, text: &[u16]) -> Result<(), Error> {
        for &character in text {
            if character == 0 || character == b'"' as u16 {
                return Err(Error::new("executable path", ERROR_INVALID_PARAMETER));
            }
            self.character(character)?;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<usize, Error> {
        let length = self.length;
        self.character(0)?;
        Ok(length)
    }
}

pub(crate) fn build_schtasks_command_line(
    install: bool,
    executable: &[u16],
    buffer: &mut [u16],
) -> Result<usize, Error> {
    let mut command = WideBuilder::new(buffer);
    command.ascii("schtasks.exe")?;
    if install {
        command.ascii(" /create /tn \"")?;
        command.ascii(TASK_NAME)?;
        command.ascii("\" /tr \"\\\"")?;
        command.wide(executable)?;
        command.ascii("\\\" run\" /sc onlogon /rl highest /f")?;
    } else {
        command.ascii(" /delete /tn \"")?;
        command.ascii(TASK_NAME)?;
        command.ascii("\" /f")?;
    }
    command.finish()
}

pub(crate) fn configure_startup_task(install: bool) -> Result<(), Error> {
    let _guard = BufferGuard::acquire()?;
    let buffers = unsafe { &mut *BUFFERS.0.get() };
    let system_path = &mut buffers.system_path;
    let system_length =
        unsafe { GetSystemDirectoryW(system_path.as_mut_ptr(), system_path.len() as Dword) };
    if system_length == 0 {
        return Err(last_error("GetSystemDirectoryW"));
    }
    if system_length as usize >= system_path.len() {
        return Err(Error::new("system directory", ERROR_INSUFFICIENT_BUFFER));
    }
    let mut application = WideBuilder::with_length(system_path, system_length as usize);
    application.ascii("\\schtasks.exe")?;
    application.finish()?;

    let command_line = &mut buffers.command_line;
    if install {
        let executable = &mut buffers.executable;
        let length = unsafe {
            GetModuleFileNameW(
                null_mut(),
                executable.as_mut_ptr(),
                executable.len() as Dword,
            )
        };
        if length == 0 {
            return Err(last_error("GetModuleFileNameW"));
        }
        if length as usize >= executable.len() {
            return Err(Error::new("executable path", ERROR_INSUFFICIENT_BUFFER));
        }
        build_schtasks_command_line(true, &executable[..length as usize], command_line)?;
    } else {
        build_schtasks_command_line(false, &[], command_line)?;
    }

    let mut startup: StartupInfoW = unsafe { core::mem::zeroed() };
    startup.cb = size_of::<StartupInfoW>() as Dword;
    let mut information: ProcessInformation = unsafe { core::mem::zeroed() };
    if unsafe {
        CreateProcessW(
            system_path.as_ptr(),
            command_line.as_mut_ptr(),
            null_mut(),
            null_mut(),
            0,
            0,
            null_mut(),
            null(),
            &mut startup,
            &mut information,
        )
    } == 0
    {
        return Err(last_error("CreateProcessW"));
    }

    let handles = ProcessHandles {
        process: information.process,
        thread: information.thread,
    };
    if unsafe { WaitForSingleObject(handles.process, INFINITE) } != WAIT_OBJECT_0 {
        return Err(last_error("WaitForSingleObject"));
    }
    let mut exit_code = 0;
    if unsafe { GetExitCodeProcess(handles.process, &mut exit_code) } == 0 {
        return Err(last_error("GetExitCodeProcess"));
    }
    if exit_code != 0 {
        return Err(Error::new("schtasks.exe", exit_code));
    }
    Ok(())
}
