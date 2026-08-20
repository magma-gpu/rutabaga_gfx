// Copyright 2026 Google
// SPDX-License-Identifier: MIT

use std::os::fd::OwnedFd;

use rustix::event::eventfd;
use rustix::event::EventfdFlags;
use rustix::io::read;
use rustix::io::write;

use crate::util::AsBorrowedDescriptor;
use crate::util::Error;
use crate::util::Handle;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;
use crate::util::MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD;

/// The end of an event that reports progress.
pub struct EventSignaler {
    descriptor: OwnedDescriptor,
}

/// The end of an event that blocks until progress is reported.
pub struct EventWaiter {
    descriptor: OwnedDescriptor,
}

/// Creates the two ends of one event.
///
/// An eventfd is symmetric and either end could do either job, but the callers
/// are not: one signals and the other waits, and which is which is decided
/// where the pair is created rather than by a later clone.
pub fn create_event_pair() -> MagmaGpuResult<(EventSignaler, EventWaiter)> {
    let owned: OwnedFd = eventfd(0, EventfdFlags::empty())?;
    let waiter = EventWaiter {
        descriptor: owned.into(),
    };
    let signaler = EventSignaler {
        descriptor: waiter.descriptor.try_clone()?,
    };
    Ok((signaler, waiter))
}

impl EventSignaler {
    pub fn add(&self, value: u64) -> MagmaGpuResult<()> {
        let _ = write(&self.descriptor, &value.to_le_bytes())?;
        Ok(())
    }

    pub fn signal(&self) -> MagmaGpuResult<()> {
        self.add(1)
    }
}

impl EventWaiter {
    /// Duplicates the waiting end.
    ///
    /// An eventfd can be read from two descriptors, so several callers can wait
    /// on one event. Platforms where waiting is a unique capability cannot do
    /// this.
    pub fn try_clone(&self) -> MagmaGpuResult<EventWaiter> {
        Ok(EventWaiter {
            descriptor: self.descriptor.try_clone()?,
        })
    }

    /// Makes an end that can signal this event.
    ///
    /// A caller that holds the waiting end may also have to report on the same
    /// event, when progress travels in either direction.
    pub fn signaler(&self) -> MagmaGpuResult<EventSignaler> {
        Ok(EventSignaler {
            descriptor: self.descriptor.try_clone()?,
        })
    }

    pub fn wait(&self) -> MagmaGpuResult<u64> {
        let mut buf = [0; 8];
        read(&self.descriptor, &mut buf)?;
        Ok(u64::from_le_bytes(buf))
    }
}

impl TryFrom<Handle> for EventSignaler {
    type Error = Error;
    fn try_from(handle: Handle) -> Result<Self, Self::Error> {
        if handle.handle_type != MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD {
            return Err(Error::InvalidMagmaHandle);
        }

        Ok(EventSignaler {
            descriptor: handle.os_handle,
        })
    }
}

impl TryFrom<Handle> for EventWaiter {
    type Error = Error;
    fn try_from(handle: Handle) -> Result<Self, Self::Error> {
        if handle.handle_type != MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD {
            return Err(Error::InvalidMagmaHandle);
        }

        Ok(EventWaiter {
            descriptor: handle.os_handle,
        })
    }
}

impl From<EventSignaler> for Handle {
    fn from(evt: EventSignaler) -> Self {
        Handle {
            os_handle: evt.descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD,
        }
    }
}

impl From<EventWaiter> for Handle {
    fn from(evt: EventWaiter) -> Self {
        Handle {
            os_handle: evt.descriptor,
            handle_type: MAGMA_GPU_HANDLE_TYPE_SIGNAL_EVENT_FD,
        }
    }
}

impl AsBorrowedDescriptor for EventSignaler {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}

impl AsBorrowedDescriptor for EventWaiter {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        &self.descriptor
    }
}
