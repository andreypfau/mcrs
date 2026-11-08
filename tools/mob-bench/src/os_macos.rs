//! The macOS counters: Mach per-CPU ticks, `proc_pid_rusage` per process, the load average.
#![expect(
    deprecated,
    reason = "libc marks its Mach bindings deprecated in favour of mach2"
)]

use std::ffi::c_void;
use std::mem::{MaybeUninit, size_of};

use crate::os::{CoreTicks, ProcessUsage};

/// The ticks each CPU spent in user, system, nice and idle state since boot, from
/// `host_processor_info(PROCESSOR_CPU_LOAD_INFO)`.
pub fn core_ticks() -> Vec<CoreTicks> {
    let mut count: libc::natural_t = 0;
    let mut info: libc::processor_info_array_t = std::ptr::null_mut();
    let mut info_count: libc::mach_msg_type_number_t = 0;
    // SAFETY: the out-pointers point to live locals; the kernel allocates the array it returns.
    let status = unsafe {
        libc::host_processor_info(
            libc::mach_host_self(),
            libc::PROCESSOR_CPU_LOAD_INFO,
            &mut count,
            &mut info,
            &mut info_count,
        )
    };
    if status != libc::KERN_SUCCESS || info.is_null() {
        return Vec::new();
    }
    let states = libc::CPU_STATE_MAX as usize;
    let length = info_count as usize;
    // SAFETY: the kernel returned `info_count` integers at `info`, `CPU_STATE_MAX` per CPU.
    let values = unsafe { std::slice::from_raw_parts(info, length) };
    let mut cores = Vec::with_capacity(count as usize);
    for cpu in values.chunks_exact(states) {
        let tick = |state: i32| cpu.get(state as usize).map_or(0, |value| *value as u32);
        cores.push(CoreTicks {
            user: u64::from(tick(libc::CPU_STATE_USER)),
            system: u64::from(tick(libc::CPU_STATE_SYSTEM)),
            nice: u64::from(tick(libc::CPU_STATE_NICE)),
            idle: u64::from(tick(libc::CPU_STATE_IDLE)),
        });
    }
    // SAFETY: the array came from the kernel in this task's address space, with this size.
    unsafe {
        let task = libc::mach_task_self();
        libc::vm_deallocate(
            task,
            info as libc::vm_address_t,
            length * size_of::<libc::integer_t>(),
        );
    }
    cores
}

/// The CPU time and resident memory of `pid`, or `None` for a process this user may not read.
pub fn process(pid: i32) -> Option<ProcessUsage> {
    let mut info = MaybeUninit::<libc::rusage_info_v2>::zeroed();
    // SAFETY: `info` is a buffer of the size the V2 flavour writes.
    let status = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V2,
            info.as_mut_ptr().cast::<libc::rusage_info_t>(),
        )
    };
    if status != 0 {
        return None;
    }
    // SAFETY: the call succeeded, so the kernel filled the buffer.
    let info = unsafe { info.assume_init() };
    let (numer, denom) = timebase();
    // `ri_user_time` is in Mach absolute time units, which are not nanoseconds on Apple silicon.
    let nanos = |ticks: u64| (u128::from(ticks) * u128::from(numer) / u128::from(denom)) as u64;
    Some(ProcessUsage {
        cpu_nanos: nanos(info.ri_user_time) + nanos(info.ri_system_time),
        rss_bytes: info.ri_resident_size,
    })
}

fn timebase() -> (u32, u32) {
    let mut info = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: `info` is a live local the call fills.
    let status = unsafe { libc::mach_timebase_info(&mut info) };
    if status != 0 || info.denom == 0 {
        return (1, 1);
    }
    (info.numer, info.denom)
}

/// Every process id the kernel lists.
pub fn pids() -> Vec<i32> {
    let mut pids = vec![0i32; 8192];
    let bytes = i32::try_from(pids.len() * size_of::<i32>()).unwrap_or(i32::MAX);
    // SAFETY: the buffer holds `bytes` bytes.
    let count = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast::<c_void>(), bytes) };
    pids.truncate(usize::try_from(count).unwrap_or(0));
    pids.retain(|pid| *pid > 0);
    pids
}

pub fn name(pid: i32) -> String {
    let mut buffer = [0u8; 256];
    // SAFETY: the buffer holds 256 bytes.
    let length = unsafe { libc::proc_name(pid, buffer.as_mut_ptr().cast::<c_void>(), 256) };
    let length = usize::try_from(length).unwrap_or(0).min(buffer.len());
    String::from_utf8_lossy(buffer.get(..length).unwrap_or_default()).into_owned()
}

/// The 1-, 5- and 15-minute load averages.
pub fn load_average() -> [f64; 3] {
    let mut load = [0.0f64; 3];
    // SAFETY: the buffer holds three doubles.
    let filled = unsafe { libc::getloadavg(load.as_mut_ptr(), 3) };
    if filled != 3 {
        return [0.0; 3];
    }
    load
}
