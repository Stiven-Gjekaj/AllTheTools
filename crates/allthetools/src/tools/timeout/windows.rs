//! The host file of `att timeout` on Windows.
//!
//! Windows has no signals. The command goes into a job object, so the stop
//! reaches each process that the command starts, and the job ends them all
//! at once.

use std::ffi::OsString;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command, ExitStatus};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
};

use super::parse::{Ended, Outcome};
use super::{Failure, Plan};

/// The time between two checks of the command.
const POLL: Duration = Duration::from_millis(10);

/// The exit code that the processes of the job get when the time runs out.
/// It is 128 and SIGTERM, as a shell on Linux reports it.
const STOPPED: u32 = 143;

struct Job(HANDLE);

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle came from CreateJobObjectW and closes one time.
        #[allow(unsafe_code)]
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub fn run(plan: &Plan, program: &OsString, rest: &[OsString]) -> Result<Outcome, Failure> {
    // SAFETY: both pointers may be null. The result is checked.
    #[allow(unsafe_code)]
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(Failure::Other(std::io::Error::last_os_error().into()));
    }
    let job = Job(job);

    let mut child = Command::new(program).args(rest).spawn()?;
    // SAFETY: both handles are open. A process that started a child in the
    // short time before this call leaves that child outside the job.
    #[allow(unsafe_code)]
    unsafe {
        AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE);
    }

    let Some(limit) = plan.limit else {
        let status = child.wait().map_err(|e| Failure::Other(e.into()))?;
        return Ok(outcome(status, false));
    };
    if let Some(status) = wait_for(&mut child, limit)? {
        return Ok(outcome(status, false));
    }
    // SAFETY: the job handle is open.
    #[allow(unsafe_code)]
    unsafe {
        TerminateJobObject(job.0, STOPPED);
    }
    let status = child.wait().map_err(|e| Failure::Other(e.into()))?;
    Ok(outcome(status, true))
}

fn wait_for(child: &mut Child, time: Duration) -> Result<Option<ExitStatus>, Failure> {
    let end = Instant::now() + time;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| Failure::Other(e.into()))? {
            return Ok(Some(status));
        }
        let now = Instant::now();
        if now >= end {
            return Ok(None);
        }
        std::thread::sleep(POLL.min(end - now));
    }
}

fn outcome(status: ExitStatus, timed_out: bool) -> Outcome {
    Outcome {
        ended: Ended::Code(status.code().unwrap_or(1)),
        timed_out,
        killed: false,
    }
}
