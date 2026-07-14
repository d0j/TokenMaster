use std::{mem::size_of, sync::OnceLock, time::Instant};

use anyhow::{Context, anyhow};
use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_NO_MORE_FILES, FILETIME},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First,
                Thread32Next,
            },
            ProcessStatus::{
                K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{
                GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetCurrentProcessId,
                GetGuiResources, GetProcessHandleCount, GetProcessTimes,
            },
        },
    },
    core::HRESULT,
};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSample {
    pub monotonic_ns: u128,
    pub private_bytes: u64,
    pub working_set_bytes: u64,
    pub handle_count: u32,
    pub user_objects: u32,
    pub gdi_objects: u32,
    pub thread_count: u32,
    pub kernel_time_100ns: u64,
    pub user_time_100ns: u64,
}

impl ProcessSample {
    pub fn capture() -> anyhow::Result<Self> {
        static START: OnceLock<Instant> = OnceLock::new();
        let monotonic_ns = START.get_or_init(Instant::now).elapsed().as_nanos().max(1);

        // SAFETY: GetCurrentProcess returns a non-owning pseudo handle valid in this process.
        let process = unsafe { GetCurrentProcess() };
        let mut memory = PROCESS_MEMORY_COUNTERS_EX {
            cb: u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS_EX>())
                .context("process memory counter size")?,
            ..Default::default()
        };
        // SAFETY: the pointer targets a live, correctly sized PROCESS_MEMORY_COUNTERS_EX value;
        // the API writes at most `memory.cb` bytes and the pseudo handle is valid for the call.
        let memory_ok = unsafe {
            K32GetProcessMemoryInfo(
                process,
                (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
                memory.cb,
            )
        };
        if !memory_ok.as_bool() {
            return Err(windows::core::Error::from_thread().into());
        }

        let mut handle_count = 0_u32;
        // SAFETY: `handle_count` is valid writable storage and `process` is a valid pseudo handle.
        unsafe { GetProcessHandleCount(process, &raw mut handle_count) }
            .context("read process handle count")?;

        let mut creation = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: every FILETIME pointer is valid for writes and the pseudo handle is valid.
        unsafe {
            GetProcessTimes(
                process,
                &raw mut creation,
                &raw mut exit,
                &raw mut kernel,
                &raw mut user,
            )
        }
        .context("read process CPU times")?;

        // SAFETY: GetGuiResources only reads the valid process pseudo handle.
        let user_objects = unsafe { GetGuiResources(process, GR_USEROBJECTS) };
        // SAFETY: GetGuiResources only reads the valid process pseudo handle.
        let gdi_objects = unsafe { GetGuiResources(process, GR_GDIOBJECTS) };
        // SAFETY: this call returns the current numeric process identifier and uses no pointers.
        let process_id = unsafe { GetCurrentProcessId() };

        Ok(Self {
            monotonic_ns,
            private_bytes: u64::try_from(memory.PrivateUsage)
                .context("private byte count conversion")?,
            working_set_bytes: u64::try_from(memory.WorkingSetSize)
                .context("working set conversion")?,
            handle_count,
            user_objects,
            gdi_objects,
            thread_count: count_threads(process_id)?,
            kernel_time_100ns: filetime_ticks(kernel),
            user_time_100ns: filetime_ticks(user),
        })
    }
}

fn count_threads(process_id: u32) -> anyhow::Result<u32> {
    // SAFETY: the API receives a fixed flag and no pointer; returned owned handle is closed below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) }
        .context("create thread snapshot")?;
    let count_result = (|| {
        let mut entry = THREADENTRY32 {
            dwSize: u32::try_from(size_of::<THREADENTRY32>()).context("thread entry size")?,
            ..Default::default()
        };
        let mut count = 0_u32;

        // SAFETY: `entry` is correctly sized and writable; `snapshot` remains live.
        match unsafe { Thread32First(snapshot, &raw mut entry) } {
            Ok(()) => loop {
                if entry.th32OwnerProcessID == process_id {
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| anyhow!("thread count overflow"))?;
                }
                // SAFETY: same valid snapshot and entry storage as Thread32First.
                match unsafe { Thread32Next(snapshot, &raw mut entry) } {
                    Ok(()) => {}
                    Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => {
                        break;
                    }
                    Err(error) => return Err(error).context("advance thread snapshot"),
                }
            },
            Err(error) if error.code() == HRESULT::from_win32(ERROR_NO_MORE_FILES.0) => {}
            Err(error) => return Err(error).context("read first thread snapshot entry"),
        }
        Ok(count)
    })();

    // SAFETY: `snapshot` is the owned handle returned above and is closed exactly once.
    let close_result = unsafe { CloseHandle(snapshot) }.context("close thread snapshot");
    match (count_result, close_result) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(count), Ok(())) => Ok(count),
    }
}

const fn filetime_ticks(value: FILETIME) -> u64 {
    ((value.dwHighDateTime as u64) << 32) | value.dwLowDateTime as u64
}
