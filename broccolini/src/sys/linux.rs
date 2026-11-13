// Copyright (C) 2026 The Magma GPU Project
// SPDX-License-Identifier: MIT

use std::ffi::CString;
use std::fmt;
use std::ptr::null_mut;
use std::sync::Once;

use magma_gpu::protocols::broccolini_protocol::BroccoliniProtocol;
use magma_gpu::protocols::ipc::Stream;
use magma_gpu::util::FromRawDescriptor;
use magma_gpu::util::OwnedDescriptor;
use magma_gpu::util::Tube;

use crate::utils::BroccoliniError;
use crate::utils::BroccoliniResult;

static INSTALL_SIGCHLD_ONCE: Once = Once::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessId(pub libc::pid_t);

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

        let mut sv = [0i32; 2];
        // SAFETY: `sv` is a valid 2-element array for `socketpair`.
        let ret = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                sv.as_mut_ptr(),
            )
        };
        if ret != 0 {
            return Err(BroccoliniError::FailedToCreateSocketpair(
                std::io::Error::last_os_error(),
            ));
        }

        // SAFETY: `sv[0]` and `sv[1]` are valid newly-created file descriptors.
        let parent_fd = unsafe { OwnedDescriptor::from_raw_descriptor(sv[0]) };
        let child_fd = unsafe { OwnedDescriptor::from_raw_descriptor(sv[1]) };

        // SAFETY: `fork()` is called from the single-threaded Zygote before initializing GPU drivers.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(BroccoliniError::FailedToForkWorker(
                std::io::Error::last_os_error(),
            ));
        }

        if pid == 0 {
            drop(parent_fd);
            // SAFETY: Resetting SIGCHLD to default in the child process is safe.
            unsafe {
                libc::signal(libc::SIGCHLD, libc::SIG_DFL);
            }
            let code = match Tube::try_from(child_fd)
                .map(Stream::new)
                .map_err(BroccoliniError::from)
                .and_then(f)
            {
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
            drop(child_fd);
            Ok(WorkerProcess {
                pid: ProcessId(pid),
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
        // SAFETY: `getpid()` is always safe to call.
        ProcessId(unsafe { libc::getpid() })
    }

    pub fn set_current_name(name: &str) {
        let proc_title = format!("rutabaga-{name}");
        let bytes = proc_title.as_bytes();
        if let Ok(c_title) = CString::new(&bytes[..bytes.len().min(15)]) {
            // SAFETY: `c_title` is a valid null-terminated string of at most 15 bytes + NUL.
            unsafe {
                let _ = libc::prctl(
                    libc::PR_SET_NAME,
                    c_title.as_ptr() as libc::c_ulong,
                    0,
                    0,
                    0,
                );
            }
        }
    }
}
