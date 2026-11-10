//! The Windows counters: per-CPU times from `NtQuerySystemInformation`, `GetProcessTimes` and
//! the working set per process. Windows has no load average.

use std::ffi::c_void;
use std::mem::size_of;

use windows_sys::Wdk::System::SystemInformation::{
    NtQuerySystemInformation, SystemProcessorPerformanceInformation,
};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::System::WindowsProgramming::SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION;

use crate::os::{CoreTicks, ProcessUsage};

// chisle: the call reports the CPUs of the caller's processor group, at most 64; more needs
// `GetSystemCpuSetInformation` or a query per group.
const MAX_CPUS: usize = 64;

/// The 100 ns intervals each CPU spent in user, kernel and idle state since boot.
pub fn core_ticks() -> Vec<CoreTicks> {
    let mut info = [SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION::default(); MAX_CPUS];
    let bytes = u32::try_from(size_of_val(&info)).unwrap_or(u32::MAX);
    let mut written = 0u32;
    // SAFETY: the buffer holds `bytes` bytes and `written` is a live local.
    let status = unsafe {
        NtQuerySystemInformation(
            SystemProcessorPerformanceInformation,
            info.as_mut_ptr().cast::<c_void>(),
            bytes,
            &mut written,
        )
    };
    if status < 0 {
        return Vec::new();
    }
    let count = written as usize / size_of::<SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION>();
    let ticks = |value: i64| u64::try_from(value).unwrap_or(0);
    info.iter()
        .take(count)
        .map(|cpu| CoreTicks {
            user: ticks(cpu.UserTime),
            // `KernelTime` includes `IdleTime`.
            system: ticks(cpu.KernelTime).saturating_sub(ticks(cpu.IdleTime)),
            nice: 0,
            idle: ticks(cpu.IdleTime),
        })
        .collect()
}

/// The CPU time and working set of `pid`, or `None` for a process this user may not open.
pub fn process(pid: i32) -> Option<ProcessUsage> {
    let pid = u32::try_from(pid).ok()?;
    // SAFETY: plain call; a null handle means failure.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }
    let usage = usage(handle);
    // SAFETY: `handle` was opened above and is closed once.
    unsafe { CloseHandle(handle) };
    usage
}

fn usage(handle: HANDLE) -> Option<ProcessUsage> {
    let mut times = [FILETIME::default(); 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: `handle` is open with query rights and the four out-pointers are live locals.
    let timed = unsafe { GetProcessTimes(handle, creation, exit, kernel, user) };
    if timed == 0 {
        return None;
    }
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).unwrap_or(u32::MAX),
        ..Default::default()
    };
    // SAFETY: `memory` is a live local of the size stated in `cb`.
    let measured = unsafe { K32GetProcessMemoryInfo(handle, &mut memory, memory.cb) };
    let nanos = |time: &FILETIME| {
        (u64::from(time.dwHighDateTime) << 32 | u64::from(time.dwLowDateTime)).saturating_mul(100)
    };
    Some(ProcessUsage {
        cpu_nanos: nanos(kernel).saturating_add(nanos(user)),
        rss_bytes: if measured == 0 {
            0
        } else {
            memory.WorkingSetSize as u64
        },
    })
}

/// Every process id in a Toolhelp snapshot.
pub fn pids() -> Vec<i32> {
    let mut pids = Vec::new();
    walk_processes(|entry| {
        if let Ok(pid) = i32::try_from(entry.th32ProcessID)
            && pid > 0
        {
            pids.push(pid);
        }
        Walk::Continue
    });
    pids
}

pub fn name(pid: i32) -> String {
    let mut name = String::new();
    walk_processes(|entry| {
        if i32::try_from(entry.th32ProcessID) != Ok(pid) {
            return Walk::Continue;
        }
        let length = entry
            .szExeFile
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(entry.szExeFile.len());
        name = String::from_utf16_lossy(entry.szExeFile.get(..length).unwrap_or_default());
        Walk::Stop
    });
    name
}

/// Windows has no load average; the report keeps zeros there.
pub fn load_average() -> [f64; 3] {
    [0.0; 3]
}

#[derive(PartialEq)]
enum Walk {
    Continue,
    Stop,
}

fn walk_processes(mut visit: impl FnMut(&PROCESSENTRY32W) -> Walk) {
    // SAFETY: plain call; `INVALID_HANDLE_VALUE` means failure.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return;
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(size_of::<PROCESSENTRY32W>()).unwrap_or(u32::MAX),
        ..Default::default()
    };
    // SAFETY: `snapshot` is open and `entry` is a live local with `dwSize` set.
    let mut found = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
    while found && visit(&entry) == Walk::Continue {
        // SAFETY: as above.
        found = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
    }
    // SAFETY: `snapshot` was opened above and is closed once.
    unsafe { CloseHandle(snapshot) };
}
