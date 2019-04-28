use std::pin::Pin;

use winapi::shared::minwindef;
use winapi::um::processthreadsapi;

use heim_common::prelude::*;

use crate::units;
use super::bindings::{winternl, IntoTime};

pub struct CpuTime {
    user: units::Time,
    system: units::Time,
    idle: units::Time,
}

impl CpuTime {
    pub fn user(&self) -> units::Time {
        self.user
    }

    pub fn system(&self) -> units::Time {
        self.system
    }

    pub fn idle(&self) -> units::Time {
        self.idle
    }
}

// https://docs.microsoft.com/en-us/windows/desktop/api/processthreadsapi/nf-processthreadsapi-getsystemtimes
pub fn time() -> impl Future<Output = Result<CpuTime>> {
    let mut user = minwindef::FILETIME::default();
    let mut kernel = minwindef::FILETIME::default();
    let mut idle = minwindef::FILETIME::default();

    let result = unsafe {
        processthreadsapi::GetSystemTimes(
            &mut idle,
            &mut kernel,
            &mut user,
        )
    };

    if result == 0 {
        future::err(Error::last_os_error())
    } else {
        let user = user.into_time();
        let idle = idle.into_time();
        // Same as `psutil` subtracting idle time
        // and leaving only busy kernel time
        let system = kernel.into_time() - idle;

        future::ok(CpuTime {
            user,
            system,
            idle,
        })
    }
}

pub fn times() -> impl Stream<Item = Result<CpuTime>> {
    future::lazy(|_| {
        let processors: Vec<winternl::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION> =
            winternl::query_system_information()?;

        let stream = stream::iter(processors).map(Ok);

        // https://github.com/rust-lang-nursery/futures-rs/issues/1444
        Ok(Box::pin(stream) as Pin<Box<dyn Stream<Item = _> + Send>>)
    })
    .unwrap_or_else(|e| {
        Box::pin(stream::once(future::err(e)))
    })
    .flatten_stream()
    .map_ok(|proc_info: winternl::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION| {
        let user = proc_info.UserTime.into_time();
        let idle = proc_info.IdleTime.into_time();
        let system = proc_info.KernelTime.into_time() - idle;

        CpuTime {
            user,
            system,
            idle,
        }
    })
}
