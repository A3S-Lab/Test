use std::ffi::c_void;
use std::io;
use std::mem::{size_of, MaybeUninit};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use tokio::process::Child;
use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

pub(super) struct Job {
    handle: OwnedHandle,
}

impl Job {
    pub(super) fn attach(child: &Child, process_id: u32) -> io::Result<Self> {
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        set_kill_on_close(&handle, true)?;
        let child_handle = child.raw_handle().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "development server exited before Job Object assignment",
            )
        })?;
        if unsafe { AssignProcessToJobObject(handle.as_raw_handle(), child_handle) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let job = Self { handle };
        if let Err(error) = resume_process(process_id) {
            let _ = job.terminate();
            return Err(error);
        }
        Ok(job)
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        if unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub(super) fn is_empty(&self) -> io::Result<bool> {
        let mut value = MaybeUninit::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>::uninit();
        let length = u32::try_from(size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>())
            .map_err(|_| io::Error::other("Windows Job accounting is too large"))?;
        if unsafe {
            QueryInformationJobObject(
                self.handle.as_raw_handle(),
                JobObjectBasicAccountingInformation,
                value.as_mut_ptr().cast::<c_void>(),
                length,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { value.assume_init() }.ActiveProcesses == 0)
    }

    pub(super) fn disarm(&self) -> io::Result<()> {
        set_kill_on_close(&self.handle, false)
    }
}

fn set_kill_on_close(handle: &OwnedHandle, enabled: bool) -> io::Result<()> {
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    if enabled {
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    }
    let length = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
        .map_err(|_| io::Error::other("Windows Job limits are too large"))?;
    if unsafe {
        SetInformationJobObject(
            handle.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast::<c_void>(),
            length,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn resume_process(process_id: u32) -> io::Result<()> {
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>())
            .map_err(|_| io::Error::other("Windows thread entry is too large"))?,
        ..Default::default()
    };
    let mut found = false;
    let mut available = unsafe { Thread32First(snapshot.as_raw_handle(), &mut entry) } != 0;
    while available {
        if entry.th32OwnerProcessID == process_id {
            let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
            if raw.is_null() {
                return Err(io::Error::last_os_error());
            }
            let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            found = true;
        }
        available = unsafe { Thread32Next(snapshot.as_raw_handle(), &mut entry) } != 0;
    }
    if !found {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "suspended development server thread was not found",
        ));
    }
    Ok(())
}
