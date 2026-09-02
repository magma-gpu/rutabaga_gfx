// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

use std::io::Error;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;

use libc::c_int;
use libc::c_void;
use libc::uintptr_t;

use rustix::event::kqueue::kqueue;
use rustix::io::fcntl_setfd;
use rustix::io::FdFlags;

use crate::util::sys::darwin::mach::mach_port_get_attributes;
use crate::util::sys::darwin::mach::mach_port_info_t;
use crate::util::sys::darwin::mach::mach_port_status_t;
use crate::util::sys::darwin::mach::mach_port_t;
use crate::util::sys::darwin::mach::mach_task_self;
use crate::util::sys::darwin::mach::KernReturnExt;
use crate::util::sys::darwin::mach::MACH_PORT_RECEIVE_STATUS;
use crate::util::sys::darwin::mach::MACH_PORT_RECEIVE_STATUS_COUNT;
use crate::util::AsRawDescriptor;
use crate::util::Error as MagmaGpuError;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;
use crate::util::WaitEvent;
use crate::util::WaitTimeout;
use crate::util::WAIT_CONTEXT_MAX;

pub struct WaitContext {
    kqueue: OwnedFd,
}

fn filter_for(descriptor: &OwnedDescriptor) -> MagmaGpuResult<i16> {
    match descriptor {
        OwnedDescriptor::Fd(_) => Ok(libc::EVFILT_READ),
        OwnedDescriptor::MachReceive(_) => Ok(libc::EVFILT_MACHPORT),
        OwnedDescriptor::MachSend(_) => Err(MagmaGpuError::WithContext(
            "cannot wait on a Mach send right: only receive rights have message queues",
        )),
    }
}

fn mach_port_has_no_senders(port: mach_port_t) -> MagmaGpuResult<bool> {
    let mut status = mach_port_status_t::default();
    let mut count = MACH_PORT_RECEIVE_STATUS_COUNT;

    // SAFETY: `status` is large enough for the flavor, and `count` says so.
    unsafe {
        mach_port_get_attributes(
            mach_task_self(),
            port,
            MACH_PORT_RECEIVE_STATUS,
            &mut status as *mut mach_port_status_t as mach_port_info_t,
            &mut count,
        )
    }
    .check()?;

    Ok(status.mps_srights == 0)
}

enum ChangeOp {
    Add { connection_id: u64 },
    Delete,
}

impl WaitContext {
    pub fn new() -> MagmaGpuResult<WaitContext> {
        let kqueue = kqueue()?;
        fcntl_setfd(&kqueue, FdFlags::CLOEXEC)?;
        Ok(WaitContext { kqueue })
    }

    fn modify(&self, descriptor: &OwnedDescriptor, op: ChangeOp) -> MagmaGpuResult<()> {
        let filter = filter_for(descriptor)?;
        let ident = descriptor.as_raw_descriptor() as uintptr_t;

        let (flags, udata) = match op {
            ChangeOp::Add { connection_id } => {
                (libc::EV_ADD | libc::EV_ENABLE, connection_id as *mut c_void)
            }
            ChangeOp::Delete => (libc::EV_DELETE, std::ptr::null_mut()),
        };

        let change = libc::kevent {
            ident,
            filter,
            flags,
            fflags: 0,
            data: 0,
            udata,
        };

        // SAFETY: one well formed change entry, no output entries requested,
        // naming something the caller owns until it is deleted.
        let ret = unsafe {
            libc::kevent(
                self.kqueue.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if ret < 0 {
            return Err(Error::last_os_error().into());
        }
        Ok(())
    }

    pub fn add(&mut self, connection_id: u64, descriptor: &OwnedDescriptor) -> MagmaGpuResult<()> {
        self.modify(descriptor, ChangeOp::Add { connection_id })
    }

    pub fn wait(&mut self, timeout: WaitTimeout) -> MagmaGpuResult<Vec<WaitEvent>> {
        let timespec = match timeout {
            WaitTimeout::Finite(duration) => Some(libc::timespec {
                tv_sec: duration.as_secs() as libc::time_t,
                tv_nsec: duration.subsec_nanos() as libc::c_long,
            }),
            WaitTimeout::NoTimeout => None,
        };

        let mut buffer: [libc::kevent; WAIT_CONTEXT_MAX] = unsafe { std::mem::zeroed() };
        let count = loop {
            // SAFETY: what `add` registered stays valid until `delete`, and
            // the output buffer holds the number of entries passed.
            let ret = unsafe {
                libc::kevent(
                    self.kqueue.as_raw_fd(),
                    std::ptr::null(),
                    0,
                    buffer.as_mut_ptr(),
                    buffer.len() as c_int,
                    timespec
                        .as_ref()
                        .map(|t| t as *const libc::timespec)
                        .unwrap_or(std::ptr::null()),
                )
            };
            if ret < 0 {
                let err = Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err.into());
            }
            break ret as usize;
        };

        buffer[..count]
            .iter()
            .map(|event| {
                let hung_up = if event.filter == libc::EVFILT_MACHPORT {
                    mach_port_has_no_senders(event.ident as mach_port_t)?
                } else {
                    (event.flags & libc::EV_EOF) != 0
                };

                Ok(WaitEvent {
                    connection_id: event.udata as u64,
                    readable: true,
                    hung_up,
                })
            })
            .collect()
    }

    pub fn delete(&mut self, descriptor: &OwnedDescriptor) -> MagmaGpuResult<()> {
        self.modify(descriptor, ChangeOp::Delete)
    }
}
