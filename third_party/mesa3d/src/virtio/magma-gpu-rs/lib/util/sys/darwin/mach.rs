// Copyright 2026 Mesa3D authors
// SPDX-License-Identifier: MIT

#![allow(non_camel_case_types)]

use std::io::Error;
use std::io::Result;
use std::mem::size_of;

use libc::c_int;

pub use mach2::bootstrap::bootstrap_check_in;
pub use mach2::bootstrap::bootstrap_look_up;
pub use mach2::bootstrap::bootstrap_port;
pub use mach2::kern_return::kern_return_t;
pub use mach2::kern_return::KERN_SUCCESS;
pub use mach2::mach_port::mach_port_allocate;
pub use mach2::mach_port::mach_port_deallocate;
pub use mach2::mach_port::mach_port_destruct;
pub use mach2::mach_port::mach_port_get_attributes;
pub use mach2::mach_port::mach_port_insert_right;
pub use mach2::mach_port::mach_port_mod_refs;
pub use mach2::mach_port::mach_port_request_notification;
pub use mach2::mach_port::mach_port_set_attributes;
pub use mach2::message::mach_msg;
pub use mach2::message::mach_msg_audit_trailer_t;
pub use mach2::message::mach_msg_bits_t;
pub use mach2::message::mach_msg_header_t;
pub use mach2::message::mach_msg_id_t;
pub use mach2::message::mach_msg_size_t;
pub use mach2::message::mach_msg_type_name_t;
pub use mach2::message::mach_msg_type_number_t;
pub use mach2::message::MACH_MSGH_BITS_COMPLEX;
pub use mach2::message::MACH_MSG_PORT_DESCRIPTOR;
pub use mach2::message::MACH_MSG_TIMEOUT_NONE;
pub use mach2::message::MACH_MSG_TYPE_COPY_SEND;
pub use mach2::message::MACH_MSG_TYPE_MAKE_SEND;
pub use mach2::message::MACH_MSG_TYPE_MAKE_SEND_ONCE;
pub use mach2::message::MACH_MSG_TYPE_MOVE_RECEIVE;
pub use mach2::message::MACH_MSG_TYPE_MOVE_SEND;
pub use mach2::message::MACH_MSG_TYPE_PORT_RECEIVE;
pub use mach2::message::MACH_RCV_MSG;
pub use mach2::message::MACH_SEND_MSG;
pub use mach2::notify::MACH_NOTIFY_NO_SENDERS;
pub use mach2::port::mach_port_info_t;
pub use mach2::port::mach_port_limits_t;
pub use mach2::port::mach_port_name_t;
pub use mach2::port::mach_port_t;
pub use mach2::port::MACH_PORT_LIMITS_INFO;
pub use mach2::port::MACH_PORT_LIMITS_INFO_COUNT;
pub use mach2::port::MACH_PORT_NULL;
pub use mach2::port::MACH_PORT_RECEIVE_STATUS;
pub use mach2::port::MACH_PORT_RIGHT_RECEIVE;
pub use mach2::port::MACH_PORT_RIGHT_SEND;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct mach_port_status_t {
    pub mps_pset: u32,
    pub mps_seqno: u32,
    pub mps_mscount: u32,
    pub mps_qlimit: u32,
    pub mps_msgcount: u32,
    pub mps_sorights: u32,
    pub mps_srights: c_int,
    pub mps_pdrequest: c_int,
    pub mps_nsrequest: c_int,
    pub mps_flags: u32,
}

pub const MACH_PORT_RECEIVE_STATUS_COUNT: mach_msg_type_number_t =
    (size_of::<mach_port_status_t>() / size_of::<c_int>()) as mach_msg_type_number_t;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct mach_msg_max_trailer_t {
    pub base: mach_msg_audit_trailer_t,
    pub msgh_context: u64,
    pub msgh_ad: c_int,
    pub msgh_labels: mach_port_name_t,
}

pub const PORT_QUEUE_LIMIT: u32 = 128;

pub const fn mach_msgh_bits(
    remote: mach_msg_type_name_t,
    local: mach_msg_type_name_t,
) -> mach_msg_bits_t {
    remote | (local << 8)
}

extern "C" {
    pub fn fileport_makeport(fd: c_int, port: *mut mach_port_t) -> kern_return_t;

    pub fn fileport_makefd(port: mach_port_t) -> c_int;
}

pub trait KernReturnExt {
    fn check(self) -> Result<()>;
}

impl KernReturnExt for kern_return_t {
    #[track_caller]
    fn check(self) -> Result<()> {
        if self == KERN_SUCCESS {
            return Ok(());
        }
        let location = std::panic::Location::caller();
        Err(Error::other(format!(
            "mach call failed at {}:{}: {:#x}",
            location.file(),
            location.line(),
            self
        )))
    }
}

#[derive(Debug)]
pub struct ReceiveRight {
    name: mach_port_t,
}

#[derive(Debug)]
pub struct SendRight {
    name: mach_port_t,
}

impl ReceiveRight {
    pub fn new() -> Result<ReceiveRight> {
        let mut name: mach_port_name_t = MACH_PORT_NULL;
        // SAFETY: `name` is a valid out parameter, and the task port is this
        // task's own, which is always valid.
        unsafe { mach_port_allocate(mach_task_self(), MACH_PORT_RIGHT_RECEIVE, &mut name) }
            .check()?;

        let right = ReceiveRight { name };
        right.set_queue_limit(PORT_QUEUE_LIMIT)?;
        Ok(right)
    }

    pub unsafe fn from_raw(name: mach_port_t) -> ReceiveRight {
        ReceiveRight { name }
    }

    pub fn into_raw(self) -> mach_port_t {
        let name = self.name;
        std::mem::forget(self);
        name
    }

    pub(super) fn as_raw(&self) -> mach_port_t {
        self.name
    }

    fn set_queue_limit(&self, limit: u32) -> Result<()> {
        let mut limits = mach_port_limits_t { mpl_qlimit: limit };
        // SAFETY: `self.name` is owned by this task, and `limits` matches the
        // flavor and the count being passed.
        unsafe {
            mach_port_set_attributes(
                mach_task_self(),
                self.name,
                MACH_PORT_LIMITS_INFO,
                &mut limits as *mut mach_port_limits_t as mach_port_info_t,
                MACH_PORT_LIMITS_INFO_COUNT,
            )
        }
        .check()
    }

    pub fn make_send(&self) -> Result<SendRight> {
        // SAFETY: `self.name` is owned by this task for as long as `self`
        // lives, and a receive right is what `MAKE_SEND` requires.
        unsafe {
            mach_port_insert_right(
                mach_task_self(),
                self.name,
                self.name,
                MACH_MSG_TYPE_MAKE_SEND,
            )
        }
        .check()?;
        // SAFETY: the right just inserted is owned by the returned value.
        Ok(unsafe { SendRight::from_raw(self.name) })
    }

    pub fn request_no_senders(&self) -> Result<()> {
        let mut previous: mach_port_t = MACH_PORT_NULL;
        // SAFETY: `self.name` names a receive right owned by this task, and
        // `previous` is a valid out parameter.
        unsafe {
            mach_port_request_notification(
                mach_task_self(),
                self.name,
                MACH_NOTIFY_NO_SENDERS,
                0,
                self.name,
                MACH_MSG_TYPE_MAKE_SEND_ONCE,
                &mut previous,
            )
        }
        .check()
    }
}

impl Drop for ReceiveRight {
    fn drop(&mut self) {
        if self.name == MACH_PORT_NULL {
            return;
        }
        // Not mach_port_destroy: a task has one name per port, so make_send
        // leaves a send right under this same name and destroy would take
        // that too. A send-right delta of 0 leaves it alone.
        //
        // SAFETY: this owns the receive right named by `self.name`, the port
        // is unguarded, and the right is destroyed once.
        unsafe {
            mach_port_destruct(mach_task_self(), self.name, 0, 0);
        }
    }
}

impl SendRight {
    pub unsafe fn from_raw(name: mach_port_t) -> SendRight {
        SendRight { name }
    }

    pub fn into_raw(self) -> mach_port_t {
        let name = self.name;
        std::mem::forget(self);
        name
    }

    pub(super) fn as_raw(&self) -> mach_port_t {
        self.name
    }

    pub fn try_clone(&self) -> Result<SendRight> {
        // SAFETY: `self.name` names a send right owned by this task, and +1
        // adds a reference to it.
        unsafe { mach_port_mod_refs(mach_task_self(), self.name, MACH_PORT_RIGHT_SEND, 1) }
            .check()?;
        // SAFETY: the reference just added is owned by the clone.
        Ok(unsafe { SendRight::from_raw(self.name) })
    }
}

impl Drop for SendRight {
    fn drop(&mut self) {
        if self.name == MACH_PORT_NULL {
            return;
        }
        // SAFETY: this owns a reference to the send right named by
        // `self.name`, and releases it once.
        unsafe {
            mach_port_deallocate(mach_task_self(), self.name);
        }
    }
}

pub fn mach_task_self() -> mach_port_t {
    // SAFETY: reading that global.
    unsafe { mach2::traps::mach_task_self() }
}
