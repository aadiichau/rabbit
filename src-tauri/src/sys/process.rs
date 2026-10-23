use std::ffi::OsStr;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use tokio::process::{Child, Command};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A child process whose entire process tree lives in its own Job Object.
/// Dropping it (or calling `terminate`) kills the tree — yt-dlp.exe is a PyInstaller
/// bootloader that spawns a second process, so killing only the direct child is not enough.
pub struct ContainedChild {
    pub child: Child,
    job: Arc<Job>,
}

impl ContainedChild {
    /// A handle that can kill the process tree while `child` is borrowed elsewhere.
    pub fn killer(&self) -> Killer {
        Killer(self.job.clone())
    }
}

#[derive(Clone)]
pub struct Killer(Arc<Job>);

impl Killer {
    pub fn kill(&self) {
        self.0.terminate();
    }
}

/// Builds a command with no console window, no inherited stdin, and piped output.
/// Arguments are always passed as an argv array; no shell is involved.
pub fn command<I, S>(program: &Path, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

pub fn spawn_contained(mut cmd: Command) -> std::io::Result<ContainedChild> {
    let job = Job::new()?;
    let child = cmd.spawn()?;
    if let Some(handle) = child.raw_handle() {
        job.assign(handle as isize);
    }
    Ok(ContainedChild { child, job: Arc::new(job) })
}

struct Job(isize);

// SAFETY: a job handle is a kernel object handle; it may be used from any thread.
unsafe impl Send for Job {}
unsafe impl Sync for Job {}

impl Job {
    fn new() -> std::io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            Ok(Job(h as isize))
        }
    }

    fn assign(&self, process: isize) {
        unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(
                self.0 as _,
                process as _,
            );
        }
    }

    fn terminate(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0 as _, 1);
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0 as _);
        }
    }
}
