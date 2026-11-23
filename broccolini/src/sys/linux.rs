// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::fmt;
use std::ptr::null_mut;
use std::sync::Once;

use magma_gpu::protocols::broccolini_protocol::BroccoliniProtocol;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::Tube;

use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;

static INSTALL_SIGCHLD_ONCE: Once = Once::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId(pub u32);

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[allow(dead_code)]
pub struct WorkerProcess {
    pid: ProcessId,
    client_desc: OwnedDescriptor,
}

#[allow(dead_code)]
impl WorkerProcess {
    pub fn new<F>(f: F) -> BroccoliniResult<Self>
    where
        F: FnOnce(Stream<BroccoliniProtocol>) -> BroccoliniResult<()>,
    {
        INSTALL_SIGCHLD_ONCE.call_once(|| {
            // SAFETY: Setting `SA_NOCLDWAIT` on `SIGCHLD` prevents zombie worker processes.
            unsafe {
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = libc::SIG_IGN;
                sa.sa_flags = libc::SA_NOCLDWAIT;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(libc::SIGCHLD, &sa, null_mut());
            }
        });

        let (child_tube, parent_fd) = Tube::pair()?;
        let parent_pid = std::process::id() as libc::pid_t;

        // SAFETY: `fork()` is called from the single-threaded Zygote before initializing GPU drivers.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(BroccoliniError::FailedToForkWorker(
                std::io::Error::last_os_error(),
            ));
        }

        if pid == 0 {
            // SAFETY: `prctl`, `getppid`, `_exit`, and `signal` are direct libc syscalls with
            // valid constant arguments (`PR_SET_PDEATHSIG` with `SIGKILL`, `SIGCHLD` with
            // `SIG_DFL`) and do not access Rust memory or non-async-signal-safe state after fork.
            unsafe {
                let ret = libc::prctl(
                    libc::PR_SET_PDEATHSIG,
                    libc::SIGKILL as libc::c_ulong,
                    0 as libc::c_ulong,
                    0 as libc::c_ulong,
                    0 as libc::c_ulong,
                );
                if ret != 0 || libc::getppid() != parent_pid {
                    libc::_exit(1);
                }
                libc::signal(libc::SIGCHLD, libc::SIG_DFL);
            }
            drop(parent_fd);
            let code = match f(Stream::new(child_tube)) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("broccolini: worker pid={} failed: {e}", Self::current_id());
                    1
                }
            };
            // SAFETY: Calling `_exit` skips C++ static destructors in forked worker processes.
            unsafe {
                libc::_exit(code);
            }
        } else {
            drop(child_tube);
            Ok(WorkerProcess {
                pid: ProcessId(pid as u32),
                client_desc: parent_fd,
            })
        }
    }

    pub fn pid(&self) -> ProcessId {
        self.pid
    }

    pub fn into_client_descriptor(self) -> OwnedDescriptor {
        self.client_desc
    }

    pub fn current_id() -> ProcessId {
        ProcessId(std::process::id())
    }

    pub fn set_current_name(name: &str) {
        let proc_title = format!("broccolini-{name}");
        let bytes = proc_title.as_bytes();
        if let Ok(c_title) = CString::new(&bytes[..bytes.len().min(15)]) {
            // SAFETY: `c_title` is a valid, null-terminated C string that remains alive
            // for the duration of the `prctl` syscall,
            unsafe {
                libc::prctl(libc::PR_SET_NAME, c_title.as_ptr(), 0, 0, 0);
            }
        }
    }
}
