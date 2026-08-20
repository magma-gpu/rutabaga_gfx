// Copyright 2026 Google
// SPDX-License-Identifier: MIT

use crate::util::AsBorrowedDescriptor;
use crate::util::Error;
use crate::util::Handle;
use crate::util::OwnedDescriptor;
use crate::util::Result as MagmaGpuResult;

/// The end of an event that reports progress.
pub struct EventSignaler;

/// The end of an event that blocks until progress is reported.
pub struct EventWaiter;

/// Creates the two ends of one event.
pub fn create_event_pair() -> MagmaGpuResult<(EventSignaler, EventWaiter)> {
    Err(Error::Unsupported)
}

impl EventSignaler {
    pub fn add(&self, _value: u64) -> MagmaGpuResult<()> {
        Err(Error::Unsupported)
    }

    pub fn signal(&self) -> MagmaGpuResult<()> {
        Err(Error::Unsupported)
    }
}

impl EventWaiter {
    pub fn try_clone(&self) -> MagmaGpuResult<EventWaiter> {
        Err(Error::Unsupported)
    }

    pub fn signaler(&self) -> MagmaGpuResult<EventSignaler> {
        Err(Error::Unsupported)
    }

    pub fn wait(&self) -> MagmaGpuResult<u64> {
        Err(Error::Unsupported)
    }
}

impl TryFrom<Handle> for EventSignaler {
    type Error = Error;
    fn try_from(_handle: Handle) -> Result<Self, Self::Error> {
        Err(Error::Unsupported)
    }
}

impl TryFrom<Handle> for EventWaiter {
    type Error = Error;
    fn try_from(_handle: Handle) -> Result<Self, Self::Error> {
        Err(Error::Unsupported)
    }
}

impl From<EventSignaler> for Handle {
    fn from(_evt: EventSignaler) -> Self {
        unimplemented!("Windows has no event")
    }
}

impl From<EventWaiter> for Handle {
    fn from(_evt: EventWaiter) -> Self {
        unimplemented!("Windows has no event")
    }
}

impl AsBorrowedDescriptor for EventSignaler {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        unimplemented!("Windows has no event")
    }
}

impl AsBorrowedDescriptor for EventWaiter {
    fn as_borrowed_descriptor(&self) -> &OwnedDescriptor {
        unimplemented!("Windows has no event")
    }
}
